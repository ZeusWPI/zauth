use regex::Regex;
use rocket::http::Status;
use rocket::request::{self, Form, FromRequest, Request};
use rocket::Outcome;
use rocket_contrib::json::Json;

use models::user::*;
use DbConn;

#[derive(Serialize)]
pub struct AuthorizationToken {
	username: String,
}

impl<'a, 'r> FromRequest<'a, 'r> for AuthorizationToken {
	type Error = String;

	fn from_request(
		request: &'a Request<'r>,
	) -> request::Outcome<AuthorizationToken, String> {
		let headers: Vec<_> = request.headers().get("Authorization").collect();
		if headers.is_empty() {
			let msg = String::from("Authorization header missing");
			return Outcome::Failure((Status::BadRequest, msg));
		} else if headers.len() > 1 {
			let msg = String::from("More than one authorization header");
			return Outcome::Failure((Status::BadRequest, msg));
		}

		let auth_header = headers[0];
		lazy_static! {
			static ref RE: Regex =
				Regex::new(r"^Bearer ([[[:alnum:]]+/=]+)$").unwrap();
		}

		if let Some(token) = RE.captures(auth_header).map(|c| c[1].to_string())
		{
			Outcome::Success(AuthorizationToken { username: token })
		} else {
			let msg = "Unable to parse tokenn".to_string();
			Outcome::Failure((Status::BadRequest, msg))
		}
	}
}

#[get("/current_user")]
pub fn current_user(
	token: AuthorizationToken,
	conn: DbConn,
) -> Json<AuthorizationToken>
{
	Json(token)
}

#[get("/users")]
pub fn list_users(conn: DbConn) -> Json<Vec<User>> {
	Json(User::all(&conn))
}

#[post("/users", data = "<user>")]
pub fn create_user(user: Form<NewUser>, conn: DbConn) -> Json<Option<User>> {
	Json(User::create(user.into_inner(), &conn))
}
