use rocket::http::Status;
use rocket::http::uri::Absolute;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use std::fmt::Debug;
use validator::ValidationErrors;

use crate::config::{AdminEmail, Config};
use crate::controllers::sessions_controller::rocket_uri_macro_new_session;
use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::{AdminSession, UserClientSession, UserSession};
use crate::errors::Either::{self, Left, Right};
use crate::errors::{InternalError, OneOf, Result, ZauthError};
use crate::mailer::Mailer;
use crate::models::client::Client;
use crate::models::role::Role;
use crate::models::user::*;
use crate::util::split_scopes;
use crate::views::accepter::Accepter;
use crate::{DbConn, util};
use askama::Template;
use chrono::{Duration, Utc};
use rocket::State;
use rocket::form::Form;
use rocket::serde::json::Json;

#[derive(Serialize)]
pub struct UserInfo {
	id: i32,
	username: String,
	admin: bool,
	full_name: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	roles: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	email: Option<String>,
	picture: String,
	sub: String,
}

impl UserInfo {
	async fn new(
		user: User,
		client: Option<Client>,
		scope: Option<String>,
		db: &DbConn,
		config: &Config,
	) -> Result<Self> {
		let scopes = split_scopes(&scope);

		let roles = if let Some(client) = &client {
			if scopes.contains(&"roles".into()) {
				Some(
					user.clone()
						.roles_for_client(client.id, db)
						.await?
						.iter()
						.map(|r| r.clone().name)
						.collect(),
				)
			} else {
				None
			}
		} else {
			Some(
				user.clone()
					.roles(db)
					.await?
					.iter()
					.map(|r| r.clone().name)
					.collect(),
			)
		};

		let email = if client.is_some() && scopes.contains(&"email".into()) {
			Some(format!("{}@{}", user.username, config.user_mail_domain))
		} else {
			None
		};

		Ok(UserInfo {
			id: user.id,
			username: user.username,
			admin: user.admin,
			full_name: user.full_name,
			roles,
			email,
			picture: format!("{}{}", config.picture_url_prefix(), user.id),
			sub: format!("{}", user.id),
		})
	}
}

#[get("/current_user", rank = 1)]
pub async fn current_user_as_client(
	session: UserClientSession,
	db: DbConn,
	config: &State<Config>,
) -> Result<Json<UserInfo>> {
	Ok(Json(
		UserInfo::new(
			session.user,
			Some(session.client),
			session.scope,
			&db,
			config,
		)
		.await?,
	))
}

#[get("/current_user", rank = 2)]
pub async fn current_user(
	session: UserSession,
	db: DbConn,
	config: &State<Config>,
) -> Result<Json<UserInfo>> {
	Ok(Json(
		UserInfo::new(session.user, None, None, &db, config).await?,
	))
}

#[get("/users/<username>")]
pub async fn show_user<'r>(
	session: UserSession,
	db: DbConn,
	username: String,
) -> Result<impl Responder<'r, 'static>> {
	// Cloning the username is necessary because it's used later
	let user = User::find_by_username(username.clone(), &db).await?;
	let user_roles = user.clone().roles(&db).await?;
	let roles = if session.user.admin {
		Role::all(&db).await?
	} else {
		vec![]
	};
	// Check whether the current session is allowed to view this user
	if session.user.admin || session.user.username == username {
		Ok(Accepter {
			html: template!("users/show.html";
							user: User = user.clone(),
							current_user: User = session.user,
							user_roles: Vec<Role> = user_roles,
							roles: Vec<Role> = roles,
							errors: Option<ValidationErrors> = None
			),
			json: Json(user),
		})
	} else {
		Err(ZauthError::not_found(&format!(
			"User with username {} not found",
			username
		)))
	}
}

#[get("/users/<username>/keys", rank = 1)]
pub async fn show_ssh_key<'r>(
	db: DbConn,
	username: String,
) -> Result<impl Responder<'r, 'static>> {
	let user = User::find_by_username(username, &db).await?;
	let mut keys = vec![];
	if let Some(ssh_keys) = user.ssh_key {
		for line in ssh_keys.lines() {
			let line = line.trim();
			if !line.is_empty() {
				keys.push(
					line.split_ascii_whitespace()
						.take(2)
						.collect::<Vec<&str>>()
						.join(" "),
				)
			}
		}
	}
	Ok(Accepter {
		html: template!("users/keys.html"; keys: String = keys.join("\n")),
		json: Json(keys),
	})
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
) -> Result<impl Responder<'r, 'static> + use<'r>> {
	let user = User::create(user.into_inner(), config.bcrypt_cost, &db).await?;
	// Cloning the username is necessary because it's used later
	Ok(Accepter {
		html: Redirect::to(uri!(show_user(user.username.clone()))),
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
		user: NewUser = NewUser {
			username: "".to_string(),
			full_name: "".to_string(),
			email: "".to_string(),
			password: "".to_string(),
			ssh_key: None,
			not_a_robot: false,
		}
	})
}

#[post("/register", data = "<user>")]
pub async fn register<'r>(
	user: Api<NewUser>,
	db: DbConn,
	conf: &'r State<Config>,
	mailer: &'r State<Mailer>,
) -> Result<Either<impl Responder<'r, 'static>, impl Responder<'r, 'static>>> {
	let new_user = user.into_inner();
	let pending = User::create_pending(new_user.clone(), &conf, &db).await;
	let full = User::pending_count(&db).await? >= conf.maximum_pending_users;
	match pending {
		Ok(user) => {
			let confirm_url = uri!(
				conf.base_url(),
				confirm_email_get(
					user.pending_email_token
						.clone()
						.unwrap_or(String::from(""))
				)
			);
			mailer.try_create(
				&user,
				String::from("[Zauth] Confirm your email"),
				template!(
				"mails/confirm_user_registration.txt";
				name: String = user.full_name.to_string(),
				confirm_url: String = confirm_url.to_string(),
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
					user: NewUser = new_user,
					errors: Option<ValidationErrors> = Some(errors.clone()),
				},
			),
			json: Custom(Status::UnprocessableEntity, Json(errors)),
		})),
		Err(other) => Err(other),
	}
}

#[put("/users/<username>", data = "<change>")]
pub async fn update_user<'r, 'o: 'r>(
	username: String,
	change: Api<UserChange>,
	session: UserSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'o>> {
	let mut user = User::find_by_username(username, &db).await?;
	if session.user.id == user.id || session.user.admin {
		match user.change_with(change.into_inner()) {
			Ok(()) => {
				let user = user.update(&db).await?;
				Ok(OneOf::One(Accepter {
					html: Redirect::to(uri!(show_user(user.username))),
					json: Custom(Status::NoContent, ()),
				}))
			},
			Err(ZauthError::ValidationError(errors)) => {
				let roles = user.clone().roles(&db).await?;
				Ok(OneOf::Two(Custom(
					Status::UnprocessableEntity,
					template! {
						"users/show.html";
						user: User = user,
						current_user: User = session.user,
						user_roles: Vec<Role> =  roles,
						roles: Vec<Role> = vec![],
						errors: Option<ValidationErrors> = Some(errors.clone()),
					},
				)))
			},
			Err(other) => Err(other),
		}
	} else {
		Ok(OneOf::Three(Custom(Status::Forbidden, ())))
	}
}

#[post("/users/<username>/admin", data = "<value>")]
pub async fn set_admin<'r>(
	username: String,
	value: Api<ChangeAdmin>,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mut user = User::find_by_username(username, &db).await?;
	user.admin = value.into_inner().admin;
	let user = user.update(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user(user.username))),
		json: Custom(Status::NoContent, ()),
	})
}

#[post("/users/<username>/change_state", data = "<value>")]
pub async fn change_state<'r>(
	username: String,
	value: Api<ChangeStatus>,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mut user = User::find_by_username(username, &db).await?;
	user.state = value.into_inner().state;
	let user = user.update(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user(user.username))),
		json: Custom(Status::NoContent, ()),
	})
}

#[post("/users/<username>/approve")]
pub async fn set_approved<'r>(
	username: String,
	_session: AdminSession,
	mailer: &'r State<Mailer>,
	conf: &'r State<Config>,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let user = User::find_by_username(username, &db).await?;
	let user = user.approve(&db).await?;

	let login_url = uri!(conf.base_url(), new_session);

	mailer
		.create(
			&user,
			String::from("[Zauth] Your account has been approved"),
			template!(
			"mails/user_approved.txt";
			name: String = user.full_name.to_string(),
			login_url: String = login_url.to_string(),
			)
			.render()
			.map_err(InternalError::from)?,
		)
		.await?;

	Ok(Accepter {
		html: Redirect::to(uri!(show_user(user.username))),
		json: Custom(Status::NoContent, ()),
	})
}

#[post("/users/<username>/reject")]
pub async fn reject<'r>(
	username: String,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let user = User::find_by_username(username, &db).await?;

	if user.state != UserState::PendingApproval {
		return Err(ZauthError::Unprocessable(String::from(
			"user is not in the pending approval state",
		)));
	}

	user.delete(&db).await?;

	Ok(Accepter {
		html: Redirect::to(uri!(list_users())),
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
) -> Result<impl Responder<'r, 'static> + use<'r>> {
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

#[get("/users/unsubscribe/<token>")]
pub fn show_confirm_unsubscribe<'r>(
	token: String,
) -> impl Responder<'r, 'static> {
	template! {
		"users/confirm_unsubscribe_form.html";
		token: String = token,
	}
}

#[derive(Debug, FromForm)]
pub struct UnsubscribeForm {
	token: String,
}

#[post("/users/unsubscribe", data = "<form>")]
pub async fn unsubscribe_user<'r>(
	form: Form<UnsubscribeForm>,
	db: DbConn,
) -> Result<Either<impl Responder<'r, 'static>, impl Responder<'r, 'static>>> {
	let user =
		User::find_by_unsubscribe_token(form.into_inner().token, &db).await?;

	if user.is_none() {
		return Ok(Either::Left(Custom(
			Status::Unauthorized,
			template!("users/unsubscribe_invalid.html"),
		)));
	}

	let mut user = user.unwrap();
	let new_token = util::random_token(32);
	user.unsubscribe_token = new_token;
	user.subscribed_to_mailing_list = false;
	user.update(&db).await?;

	Ok(Either::Right(template!("users/unsubscribed.html")))
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
	token: String,
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
		let change = ChangePassword {
			password: form.new_password,
		};
		let changed = user.change_password(change, conf, &db).await;
		match changed {
			Ok(user) => {
				let body = template!(
					"mails/password_reset_success.txt";
					name: String = user.username.to_string(),
				)
				.render()
				.map_err(InternalError::from)?;
				mailer
					.create(
						&user,
						String::from("[Zauth] Your password has been reset"),
						body,
					)
					.await?;
				Ok(OneOf::One(
					template! { "users/reset_password_success.html" },
				))
			},

			Err(ZauthError::ValidationError(errors)) => Ok(OneOf::Two(Custom(
				Status::UnprocessableEntity,
				template! {
					"users/reset_password_form.html";
					token: String = form.token,
					errors: Option<ValidationErrors> = Some(errors.clone()),
				},
			))),
			Err(other) => Err(other),
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
	mailer: &State<Mailer>,
	admin_email: &State<AdminEmail>,
	conf: &'r State<Config>,
	db: DbConn,
) -> Result<
	Either<
		impl Responder<'r, 'static> + use<'r>,
		impl Responder<'r, 'static> + use<'r>,
	>,
> {
	if let Some(user) =
		User::find_by_email_token(form.token.clone(), &db).await?
	{
		let user = user.confirm_email(&db).await?;

		let user_list_url = uri!(conf.base_url(), list_users);
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

		Ok(Either::Left(template! {
			"users/confirm_email_success.html";
			user: User = user,
		}))
	} else {
		Ok(Either::Right(
			template! {"users/confirm_email_invalid.html"},
		))
	}
}

#[post("/users/<username>/roles", data = "<role_id>")]
pub async fn add_role<'r>(
	username: String,
	role_id: Form<i32>,
	db: DbConn,
	_session: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(*role_id, &db).await?;
	let user_result = User::find_by_username(username.clone(), &db).await?;
	role.add_user(user_result.id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user(username))),
		json: Custom(Status::NoContent, ()),
	})
}

#[delete("/users/<username>/roles/<role_id>")]
pub async fn delete_role<'r>(
	role_id: i32,
	username: String,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let user_result = User::find_by_username(username.clone(), &db).await?;
	role.remove_user(user_result.id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_user(username))),
		json: Custom(Status::NoContent, ()),
	})
}
