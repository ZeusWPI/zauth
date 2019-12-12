use rocket_contrib::json::Json;

use crate::ephemeral::authorization_token::AuthorizationToken;
use crate::ephemeral::from_api::Api;
use crate::models::user::*;
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
pub fn list_users(conn: DbConn) -> Json<Vec<User>> {
	Json(User::all(&conn))
}

#[post("/users", data = "<user>")]
pub fn create_user(user: Api<NewUser>, conn: DbConn) -> Json<Option<User>> {
	Json(User::create(user.into_inner(), &conn))
}
