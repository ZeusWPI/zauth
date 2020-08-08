use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket_contrib::json::Json;

use std::fmt::Debug;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::{AdminSession, UserSession};
use crate::errors::{AuthenticationError, Result, ZauthError};
use crate::models::user::*;
use crate::views::accepter::Accepter;
use crate::DbConn;
use crate::Either::{self, Left, Right};

#[get("/current_user")]
pub fn current_user(session: UserSession) -> Json<User> {
	Json(session.user)
}

#[get("/users/<id>")]
pub fn show_user(
	session: UserSession,
	conn: DbConn,
	id: i32,
) -> Result<impl Responder<'static>> {
	let user = User::find(id, &conn)?;
	if session.user.admin || session.user.id == user.id {
		Ok(Accepter {
			html: template!("users/show.html"; user: User = user.clone()),
			json: Json(user),
		})
	} else {
		Err(ZauthError::from(AuthenticationError::Unauthorized(
			format!("client with id {} is not authorized on this server", id),
		)))
	}
}

#[get("/users")]
pub fn list_users(
	session: UserSession,
	conn: DbConn,
) -> Result<impl Responder<'static>> {
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
	conn: DbConn,
) -> Result<impl Responder<'static>> {
	let user = User::create(user.into_inner(), &conn)?;
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
	conn: DbConn,
) -> Result<
	Either<impl Responder<'static>, Custom<impl Debug + Responder<'static>>>,
> {
	let mut user = User::find(id, &conn)?;
	if session.user.id == user.id || session.user.admin {
		user.change_with(change.into_inner())?;
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
) -> Result<impl Responder<'static>> {
	let mut user = User::find(id, &conn)?;
	dbg!(&user);
	dbg!(&value);
	user.admin = value.into_inner().admin;
	dbg!(&user);
	let user = user.update(&conn)?;
	dbg!(&user);
	Ok(Accepter {
		html: Redirect::to(uri!(show_user: user.id)),
		json: Custom(Status::NoContent, ()),
	})
}
