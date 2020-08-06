use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket_contrib::json::Json;

use std::fmt::Debug;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::{AdminSession, UserSession};
use crate::models::user::*;
use crate::views::accepter::Accepter;
use crate::DbConn;

#[get("/current_user")]
pub fn current_user(session: UserSession) -> Json<User> {
	Json(session.user)
}

#[get("/users/<id>")]
pub fn show_user(
	session: UserSession,
	conn: DbConn,
	id: i32,
) -> Option<impl Responder<'static>>
{
	if let Some(user) = User::find(id, &conn) {
		if session.user.admin || session.user.id == user.id {
			return Some(Accepter {
				html: template!("users/show.html"; user: User = user.clone()),
				json: Json(user),
			});
		}
	}
	None
}

#[get("/users")]
pub fn list_users(
	session: UserSession,
	conn: DbConn,
) -> impl Responder<'static>
{
	let users = User::all(&conn);
	Accepter {
		html: template! {
			"users/index.html";
			users: Vec<User> = users.clone(),
			current_user: User = session.user,
		},
		json: Json(users),
	}
}

#[post("/users", data = "<user>")]
pub fn create_user(
	user: Api<NewUser>,
	conn: DbConn,
) -> Option<impl Responder<'static>>
{
	let user = User::create(&conn, user.into_inner())?;
	Some(Accepter {
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
) -> Option<
	Result<impl Responder<'static>, Custom<impl Debug + Responder<'static>>>,
>
{
	let mut user = User::find(id, &conn)?;
	if session.user.id == user.id || session.user.admin {
		user.change_with(change.into_inner())?;
		let user = user.update(&conn)?;
		Some(Ok(Accepter {
			html: Redirect::to(uri!(show_user: user.id)),
			json: Custom(Status::NoContent, ()),
		}))
	} else {
		Some(Err(Custom(Status::Forbidden, ())))
	}
}

#[post("/users/<id>/admin", data = "<value>")]
pub fn set_admin(
	id: i32,
	value: Api<ChangeAdmin>,
	_session: AdminSession,
	conn: DbConn,
) -> Option<impl Responder<'static>>
{
	let mut user = User::find(id, &conn)?;
	dbg!(&user);
	dbg!(&value);
	user.admin = value.into_inner().admin;
	dbg!(&user);
	let user = user.update(&conn)?;
	dbg!(&user);
	Some(Accepter {
		html: Redirect::to(uri!(show_user: user.id)),
		json: Custom(Status::NoContent, ()),
	})
}
