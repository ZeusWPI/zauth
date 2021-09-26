use rocket::http::uri::Absolute;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use std::fmt::Debug;
use validator::ValidationErrors;

use crate::config::{AdminEmail, Config};
use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::{
	AdminSession, ClientOrUserSession, ClientSession, UserSession,
};
use crate::errors::Either::{self, Left, Right};
use crate::errors::{InternalError, OneOf, Result, ZauthError};
use crate::mailer::Mailer;
use crate::models::user::*;
use crate::views::accepter::Accepter;
use crate::{util, DbConn};
use askama::Template;
use chrono::{Duration, Utc};
use rocket::form::Form;
use rocket::serde::json::Json;
use rocket::State;

#[get("/current_user")]
pub fn current_user(session: ClientOrUserSession) -> Json<User> {
	Json(session.user)
}

#[get("/current_user")]
pub fn current_user_as_client(session: ClientSession) -> Json<User> {
	Json(session.user)
}

#[get("/users/<id>")]
pub async fn show_user<'r>(
	session: UserSession,
	db: DbConn,
	id: i32,
) -> Result<impl Responder<'r, 'static>> {
	let user = User::find(id, &db).await?;
	// Check whether the current session is allowed to view this user
	if session.user.admin || session.user.id == id {
		Ok(Accepter {
			html: template!("users/show.html";
							user: User = user.clone(),
							current_user: User = session.user,
			),
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
pub async fn list_users<'r>(
	session: AdminSession,
	db: DbConn,
	conf: &'r State<Config>,
) -> Result<impl Responder<'r, 'static>> {
	let users = User::all(&db).await?;
	let full = User::pending_count(&db).await? >= conf.maximum_pending_users;
	let users_pending_for_approval: Vec<User> =
		User::find_by_pending(&db).await?;
	Ok(Accepter {
		html: template! {
			"users/index.html";
			users: Vec<User> = users.clone(),
			current_user: User = session.admin,
			registrations_full: bool = full,
			users_pending_for_approval: Vec<User> = users_pending_for_approval.clone(),
		},
		json: Json(users),
	})
}

#[get("/users/new")]
pub fn create_user_page<'r>(
	session: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	Ok(template! { "users/new_user.html";
		current_user: User = session.admin,
	})
}

#[post("/users", data = "<user>")]
pub async fn create_user<'r>(
	_session: AdminSession,
	user: Api<NewUser>,
	db: DbConn,
	config: &State<Config>,
) -> Result<impl Responder<'r, 'static>> {
	let user = User::create(user.into_inner(), config.bcrypt_cost, &db)
		.await
		.map_err(ZauthError::from)?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user(user.id))),
		json: Json(user),
	})
}

#[get("/register")]
pub async fn register_page<'r>(
	db: DbConn,
	conf: &'r State<Config>,
) -> Result<impl Responder<'r, 'static>> {
	let full = User::pending_count(&db).await? >= conf.maximum_pending_users;
	Ok(template! {
		"users/registration_form.html";
		registrations_full: bool = full,
		errors: Option<ValidationErrors> = None,
	})
}

#[post("/register", data = "<user>")]
pub async fn register<'r>(
	user: Api<NewUser>,
	db: DbConn,
	admin_email: &'r State<AdminEmail>,
	conf: &'r State<Config>,
	mailer: &'r State<Mailer>,
) -> Result<Either<impl Responder<'r, 'static>, impl Responder<'r, 'static>>> {
	let pending = User::create_pending(user.into_inner(), &conf, &db).await;
	let full = User::pending_count(&db).await? >= conf.maximum_pending_users;
	match pending {
		Ok(user) => {
			let user_list_url = uri!(list_users);
			mailer.try_create(
				admin_email.0.clone(),
				String::from("[Zauth] New user registration"),
				template!(
				"mails/new_user_registration.txt";
				name: String = user.username.to_string(),
				user_list_url: String = user_list_url.to_string(),
				)
				.render()
				.map_err(InternalError::from)?,
			)?;

			Ok(Left(Accepter {
				html: Custom(
					Status::Created,
					template!("users/registration_success.html"),
				),
				json: Custom(Status::Created, Json(user)),
			}))
		},
		Err(ZauthError::ValidationError(errors)) => Ok(Right(Accepter {
			html: Custom(
				Status::UnprocessableEntity,
				template! {
					"users/registration_form.html";
					registrations_full: bool = full,
					errors: Option<ValidationErrors> = Some(errors.clone()),
				},
			),
			json: Custom(Status::UnprocessableEntity, Json(errors)),
		})),
		Err(other) => Err(other),
	}
}

#[put("/users/<id>", data = "<change>")]
pub async fn update_user<'r>(
	id: i32,
	change: Api<UserChange>,
	session: UserSession,
	db: DbConn,
) -> Result<
	Either<
		impl Responder<'r, 'static>,
		Custom<impl Debug + Responder<'r, 'static>>,
	>,
> {
	let mut user = User::find(id, &db).await?;
	if session.user.id == user.id || session.user.admin {
		user.change_with(change.into_inner())?;
		let user = user.update(&db).await?;
		Ok(Left(Accepter {
			html: Redirect::to(uri!(show_user(user.id))),
			json: Custom(Status::NoContent, ()),
		}))
	} else {
		Ok(Right(Custom(Status::Forbidden, ())))
	}
}

#[post("/users/<id>/admin", data = "<value>")]
pub async fn set_admin<'r>(
	id: i32,
	value: Api<ChangeAdmin>,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mut user = User::find(id, &db).await?;
	user.admin = value.into_inner().admin;
	let user = user.update(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user(user.id))),
		json: Custom(Status::NoContent, ()),
	})
}

#[post("/users/<id>/approve")]
pub async fn set_approved<'r>(
	id: i32,
	_session: AdminSession,
	mailer: &'r State<Mailer>,
	conf: &'r State<Config>,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let user = User::find(id, &db).await?;
	let user = user.approve(&conf, &db).await?;
	let confirm_url = uri!(
		conf.base_url(),
		confirm_email_get(
			user.pending_email_token.clone().unwrap_or(String::from(""))
		)
	);
	mailer.try_create(
		&user,
		String::from("[Zauth] Confirm your registration"),
		template!(
		"mails/confirm_user_registration.txt";
		name: String = user.full_name.to_string(),
		confirm_url: String = confirm_url.to_string(),
		)
		.render()
		.map_err(InternalError::from)?,
	)?;

	Ok(Accepter {
		html: Redirect::to(uri!(show_user(user.id))),
		json: Custom(Status::NoContent, ()),
	})
}

#[get("/users/forgot_password")]
pub fn forgot_password_get<'r>() -> impl Responder<'r, 'static> {
	template! { "users/forgot_password.html" }
}

#[derive(Debug, FromForm, Deserialize)]
pub struct ResetPassword {
	for_email: String,
}

#[post("/users/forgot_password", data = "<value>")]
pub async fn forgot_password_post<'r>(
	value: Form<ResetPassword>,
	conf: &State<Config>,
	db: DbConn,
	mailer: &State<Mailer>,
) -> Result<impl Responder<'r, 'static>> {
	let for_email = value.into_inner().for_email;

	let user = match User::find_by_email(for_email.to_owned(), &db).await {
		Ok(user) if user.is_active() => Ok(Some(user)),
		Ok(_user) => Ok(None),
		Err(ZauthError::NotFound(_)) => Ok(None),
		Err(other) => Err(other),
	}?;

	if let Some(mut user) = user {
		let token = util::random_token(32);
		user.password_reset_token = Some(token.clone());
		user.password_reset_expiry =
			Some(Utc::now().naive_utc() + Duration::days(1));
		let user = user.update(&db).await?;
		let base_url = Absolute::parse(&conf.base_url).expect("Valid base_url");
		let reset_url = uri!(base_url, reset_password_get(token));
		mailer.try_create(
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
pub fn reset_password_get<'r>(token: String) -> impl Responder<'r, 'static> {
	template! {
		"users/reset_password_form.html";
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
pub async fn reset_password_post<'r, 'o: 'r>(
	form: Form<PasswordReset>,
	db: DbConn,
	conf: &'r State<Config>,
	mailer: &'r State<Mailer>,
) -> Result<impl Responder<'r, 'o>> {
	let form = form.into_inner();
	if let Some(user) =
		User::find_by_password_token(form.token.to_owned(), &db).await?
	{
		let changed = user
			.change_password(&form.new_password, conf.bcrypt_cost, &db)
			.await;
		match changed {
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
			Err(err) => {
				let template = template! {
					"users/reset_password_form.html";
					token: String = form.token,
					errors: Option<String> = Some(err.to_string()),
				};
				Ok(OneOf::Two(Custom(Status::UnprocessableEntity, template)))
			},
		}
	} else {
		let template = template! { "users/reset_password_invalid.html" };
		Ok(OneOf::Three(Custom(Status::Forbidden, template)))
	}
}

#[get("/users/confirm/<token>")]
pub fn confirm_email_get<'r>(token: String) -> impl Responder<'r, 'static> {
	template! {
		"users/confirm_email_form.html";
		token: String = token,
	}
}

#[derive(Debug, FromForm)]
pub struct EmailConfirmation {
	token: String,
}

#[post("/users/confirm", data = "<form>")]
pub async fn confirm_email_post<'r>(
	form: Form<EmailConfirmation>,
	db: DbConn,
) -> Result<Either<impl Responder<'r, 'static>, impl Responder<'r, 'static>>> {
	if let Some(user) =
		User::find_by_email_token(form.token.clone(), &db).await?
	{
		let user = user.confirm_email(&db).await?;
		Ok(Either::Left(template! {
			"users/confirm_email_success.html";
			email: String = user.email,
		}))
	} else {
		Ok(Either::Right(
			template! {"users/confirm_email_invalid.html"},
		))
	}
}
