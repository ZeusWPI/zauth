#![feature(decl_macro, proc_macro_hygiene)]
#![recursion_limit = "26"]

extern crate bcrypt;
extern crate chrono;
extern crate rand;
extern crate regex;
extern crate rocket_http_authentication;

#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

pub mod models;
pub mod oauth;
pub mod token_store;
pub mod util;

use models::user::*;
use rocket::Rocket;
use rocket_contrib::templates::Template;
use token_store::TokenStore;

use self::regex::Regex;
use diesel::SqliteConnection;
use rocket::fairing::AdHoc;
use rocket::http::Status;
use rocket::request::{self, Form, FromRequest, Request};
use rocket::Outcome;
use rocket_contrib::json::Json;

// Embed diesel migrations (provides embedded_migrations::run())
embed_migrations!();

#[database("sqlite_database")]
pub struct DbConn(SqliteConnection);

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
	""
}

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

pub fn rocket() -> Rocket {
	let rocket = rocket::ignite();
	rocket
		.mount(
			"/",
			routes![
				favicon,
				current_user,
				create_user,
				users,
				oauth::authorize,
				oauth::authorize_parse_failed,
				oauth::login_get,
				oauth::login_post,
				oauth::grant_get,
				oauth::grant_post,
				oauth::token
			],
		)
		.attach(DbConn::fairing())
		.manage(TokenStore::new())
		.attach(Template::fairing())
		.attach(AdHoc::on_attach("Database Migrations", |rocket| {
			let conn = DbConn::get_one(&rocket).expect("database connection");
			match embedded_migrations::run(&*conn) {
				Ok(()) => Ok(rocket),
				Err(e) => {
					eprintln!("Failed to run database migrations: {:?}", e);
					Err(rocket)
				},
			}
		}))
}

#[get("/current_user")]
pub fn current_user(token: AuthorizationToken) -> Json<AuthorizationToken> {
	Json(token)
}

#[get("/users")]
pub fn users(conn: DbConn) -> Json<Vec<User>> {
	Json(User::all(&conn))
}

#[post("/users", data = "<user>")]
pub fn create_user(user: Form<NewUser>, conn: DbConn) -> Json<Option<User>> {
	Json(User::create(user.into_inner(), &conn))
}
