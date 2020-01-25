use rocket::response::Responder;
use rocket_contrib::json::Json;

use crate::ephemeral::authorization_token::AuthorizationToken;
use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::UserSession;
use crate::models::user::*;
use crate::views::accepter::Accepter;
use crate::DbConn;

#[get("/current_user")]
pub fn current_user(
	token: AuthorizationToken,
	_conn: DbConn,
) -> Json<AuthorizationToken>
{
	Json(token)
}

#[get("/users")]
pub fn list_users(
	session: UserSession,
	conn: DbConn,
) -> impl Responder<'static>
{
	let users = User::all(&conn);
	Accepter {
		html: template!(
		"users/index";
		users: Vec<User> = users.clone(),
		current_user: User = session.user,
		),
		json: Json(users),
	}
}

#[post("/users", data = "<user>")]
pub fn create_user(user: Api<NewUser>, conn: DbConn) -> Json<Option<User>> {
	Json(User::create(user.into_inner(), &conn))
}
