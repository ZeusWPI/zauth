use rocket::response::{Redirect, Responder};
use rocket_contrib::json::Json;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::UserSession;
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
	let user = User::create(user.into_inner(), &conn)?;
	Some(Accepter {
		html: Redirect::to(uri!(show_user: user.id)),
		json: Json(user),
	})
}
