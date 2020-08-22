use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket_contrib::json::Json;

use std::fmt::Debug;

use crate::config::Config;
use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::{AdminSession, UserSession};
use crate::errors::Either::{self, Left, Right};
use crate::errors::{InternalError, OneOf, Result, ZauthError};
use crate::mailer::Mailer;
use crate::models::user::*;
use crate::views::accepter::Accepter;
use crate::{util, DbConn};
use askama::Template;
use chrono::{Duration, Utc};
use rocket::request::Form;
use rocket::State;

#[get("/current_user")]
pub fn current_user(session: UserSession) -> Json<User> {
	Json(session.user)
}

#[get("/users/<id>")]
pub fn show_user(
	session: UserSession,
	conn: DbConn,
	id: i32,
) -> Result<impl Responder<'static>>
{
	let user = User::find(id, &conn)?;
	println!("user {:?} vs session {:?}", user, session);
	if session.user.admin || session.user.id == id {
		Ok(Accepter {
			html: template!("users/show.html"; user: User = user.clone()),
			json: Json(user),
		})
	} else {
		Err(ZauthError::not_found(&format!(
			"User with id {} not found",
			id
		)))
	}
}

#[get("/users")]
pub fn list_users(
	session: UserSession,
	conn: DbConn,
) -> Result<impl Responder<'static>>
{
	let users = User::all(&conn)?;
	Ok(Accepter {
		html: template! {
			"users/index.html";
			users: Vec<User> = users.clone(),
			current_user: User = session.user,
		},
		json: Json(users),
	})
}

#[post("/users", data = "<user>")]
pub fn create_user(
	user: Api<NewUser>,
	conf: State<Config>,
	conn: DbConn,
) -> Result<impl Responder<'static>>
{
	let user = User::create(user.into_inner(), conf.bcrypt_cost, &conn)
		.map_err(ZauthError::from)?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user: user.id)),
		json: Json(user),
	})
}

#[put("/users/<id>", data = "<change>")]
pub fn update_user(
	id: i32,
	change: Api<UserChange>,
	session: UserSession,
	conf: State<Config>,
	conn: DbConn,
) -> Result<
	Either<impl Responder<'static>, Custom<impl Debug + Responder<'static>>>,
>
{
	let mut user = User::find(id, &conn)?;
	if session.user.id == user.id || session.user.admin {
		user.change_with(change.into_inner(), conf.bcrypt_cost)?;
		let user = user.update(&conn)?;
		Ok(Left(Accepter {
			html: Redirect::to(uri!(show_user: user.id)),
			json: Custom(Status::NoContent, ()),
		}))
	} else {
		Ok(Right(Custom(Status::Forbidden, ())))
	}
}

#[post("/users/<id>/admin", data = "<value>")]
pub fn set_admin(
	id: i32,
	value: Api<ChangeAdmin>,
	_session: AdminSession,
	conn: DbConn,
) -> Result<impl Responder<'static>>
{
	let mut user = User::find(id, &conn)?;
	user.admin = value.into_inner().admin;
	let user = user.update(&conn)?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user: user.id)),
		json: Custom(Status::NoContent, ()),
	})
}

#[get("/users/forgot_password")]
pub fn forgot_password_get() -> impl Responder<'static> {
	template! { "users/forgot_password.html" }
}

#[derive(Debug, FromForm, Deserialize)]
pub struct ResetPassword {
	for_email: String,
}

#[post("/users/forgot_password", data = "<value>")]
pub fn forgot_password_post(
	value: Form<ResetPassword>,
	conn: DbConn,
	mailer: State<Mailer>,
) -> Result<impl Responder<'static>>
{
	let for_email = value.into_inner().for_email;

	let user = match User::find_by_email(&for_email, &conn) {
		Ok(user) if user.is_active() => Ok(Some(user)),
		Ok(_user) => Ok(None),
		Err(ZauthError::NotFound(_)) => Ok(None),
		Err(other) => Err(other),
	}?;

	if let Some(mut user) = user {
		user.password_reset_token = Some(util::random_token(32));
		user.password_reset_expiry =
			Some(Utc::now().naive_utc() + Duration::days(1));
		let user = user.update(&conn)?;

		let token = user.password_reset_token.as_ref().unwrap();
		let reset_url = uri!(reset_password_get: token);
		mailer.create(
			&user,
			String::from("[Zauth] You've requested a password reset"),
			template!(
				"mails/password_reset_token.txt";
				name: String = user.username.to_string(),
				reset_url: String = reset_url.to_string(),
			)
			.render()
			.map_err(InternalError::from)?,
		)?
	};

	Ok(template! {
		"users/reset_link_sent.html";
		email: String = for_email
	})
}

#[get("/users/reset_password/<token>")]
pub fn reset_password_get(token: String) -> impl Responder<'static> {
	template! {
		"users/forgot_password.html";
		token: String = token,
		errors: Option<String> = None,
	}
}

#[derive(Debug, FromForm)]
pub struct PasswordReset {
	token:        String,
	new_password: String,
}

#[post("/users/reset_password", data = "<form>")]
pub fn reset_password_post(
	form: Form<PasswordReset>,
	conn: DbConn,
	conf: State<Config>,
	mailer: State<Mailer>,
) -> Result<impl Responder<'static>>
{
	let form = form.into_inner();
	if let Some(user) = User::find_by_token(&form.token, &conn)? {
		match user.change_password(&form.new_password, conf.bcrypt_cost, &conn)
		{
			Ok(user) => {
				let body = template!(
					"mails/password_reset_success.txt";
					name: String = user.username.to_string(),
				)
				.render()
				.map_err(InternalError::from)?;
				mailer.create(
					&user,
					String::from("[Zauth] Your password has been reset"),
					body,
				)?;
				Ok(OneOf::One(
					template! { "users/reset_password_success.html" },
				))
			},
			Err(err) => Ok(OneOf::Two(template! {
					"users/reset_password_form.html";
					token: String = form.token,
					errors: Option<String> = Some(err.to_string()),
			})),
		}
	} else {
		Ok(OneOf::Three(
			template! { "users/reset_password_expired.html" },
		))
	}
}
