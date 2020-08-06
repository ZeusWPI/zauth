use chrono::{DateTime, Duration, Local};
use rocket::http::{Cookie, Cookies, Status};
use rocket::request::{FromRequest, Outcome, Request};
use std::str::FromStr;

use crate::errors::{Result, ZauthError};
use crate::models::user::User;
use crate::DbConn;

pub const SESSION_VALIDITY_MINUTES: i64 = 59;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
	user_id: i32,
	expiry: DateTime<Local>,
}

impl Session {
	pub fn new(user: User) -> Session {
		Session {
			user_id: user.id,
			expiry: Local::now() + Duration::minutes(SESSION_VALIDITY_MINUTES),
		}
	}

	pub fn add_to_cookies(user: User, cookies: &mut Cookies) {
		let session = Session::new(user);
		let session_str = serde_urlencoded::to_string(session).unwrap();
		let session_cookie = Cookie::new("session", session_str);
		cookies.add_private(session_cookie);
	}

	pub fn destroy(cookies: &mut Cookies) {
		cookies.remove_private(Cookie::named("session"))
	}

	pub fn user(&self, conn: &DbConn) -> Result<User> {
		if Local::now() > self.expiry {
			Err(anyhow!("Session expired: {:?}", self).into())
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
			.get_private("session")
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
