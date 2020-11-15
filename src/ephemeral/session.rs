use chrono::{DateTime, Duration, Local};
use rocket::http::{Cookie, Cookies, Status};
use rocket::request::{FromRequest, Outcome, Request};
use std::str::FromStr;

use crate::controllers::sessions_controller::rocket_uri_macro_new_session;
use crate::errors::{Result, ZauthError};
use crate::models::user::User;
use crate::DbConn;
use rocket::http::uri::Origin;
use rocket::response::Redirect;

pub const SESSION_VALIDITY_MINUTES: i64 = 59;
const REDIRECT_COOKIE: &str = "ZAUTH_REDIRECT";
const SESSION_COOKIE: &str = "ZAUTH_SESSION";

pub fn ensure_logged_in_and_redirect(
	mut cookies: Cookies,
	uri: Origin,
) -> Redirect
{
	cookies.add_private(Cookie::new(REDIRECT_COOKIE, uri.to_string()));
	Redirect::to(uri!(new_session))
}

pub fn stored_redirect_or(mut cookies: Cookies, fallback: Origin) -> Redirect {
	let location: Origin =
		if let Some(cookie) = cookies.get_private(REDIRECT_COOKIE) {
			let stored = Origin::parse_owned(String::from(cookie.value())).ok();
			cookies.remove_private(cookie);
			stored.unwrap_or(fallback)
		} else {
			fallback
		};
	Redirect::to(location.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
	user_id: i32,
	expiry:  DateTime<Local>,
}

impl Session {
	pub fn new(user: User) -> Session {
		Session {
			user_id: user.id,
			expiry:  Local::now() + Duration::minutes(SESSION_VALIDITY_MINUTES),
		}
	}

	pub fn login(user: User, cookies: &mut Cookies) {
		let session = Session::new(user);
		let session_str = serde_urlencoded::to_string(session).unwrap();
		let session_cookie = Cookie::new(SESSION_COOKIE, session_str);
		cookies.add_private(session_cookie);
	}

	pub fn destroy(cookies: &mut Cookies) {
		cookies.remove_private(Cookie::named(SESSION_COOKIE))
	}

	pub fn user(&self, conn: &DbConn) -> Result<User> {
		if Local::now() > self.expiry {
			Err(ZauthError::expired())
		} else {
			User::find(self.user_id, conn)
		}
	}
}

impl FromStr for Session {
	type Err = serde_urlencoded::de::Error;

	fn from_str(cookie: &str) -> std::result::Result<Session, Self::Err> {
		serde_urlencoded::from_str(cookie)
	}
}

impl<'a, 'r> FromRequest<'a, 'r> for Session {
	type Error = &'static str;

	fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
		let session = request
			.cookies()
			.get_private(SESSION_COOKIE)
			.map(|cookie| Session::from_str(cookie.value()));
		match session {
			Some(Ok(session)) => Outcome::Success(session),
			_ => Outcome::Failure((Status::Unauthorized, "invalid session")),
		}
	}
}

#[derive(Debug)]
pub struct UserSession {
	pub user: User,
}

impl<'a, 'r> FromRequest<'a, 'r> for UserSession {
	type Error = &'static str;

	fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
		let session = request.guard::<Session>()?;
		let db = request.guard::<DbConn>().map_failure(|_| {
			(Status::InternalServerError, "could not connect to database")
		})?;
		match session.user(&db) {
			Ok(user) => Outcome::Success(UserSession { user }),
			_ => Outcome::Failure((
				Status::InternalServerError,
				"user not found for valid session",
			)),
		}
	}
}

#[derive(Debug)]
pub struct AdminSession {
	pub admin: User,
}

impl<'a, 'r> FromRequest<'a, 'r> for AdminSession {
	type Error = &'static str;

	fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
		let user: User = request.guard::<UserSession>()?.user;
		if user.admin {
			Outcome::Success(AdminSession { admin: user })
		} else {
			Outcome::Failure((Status::Forbidden, "user is not an admin"))
		}
	}
}
