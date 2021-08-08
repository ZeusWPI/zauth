use chrono::{DateTime, Duration, Local};
use rocket::http::{Cookie, CookieJar, Status};
use rocket::outcome::try_outcome;
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
	cookies: &CookieJar,
	uri: Origin,
) -> Redirect {
	cookies.add_private(Cookie::new(REDIRECT_COOKIE, uri.to_string()));
	Redirect::to(uri!(new_session))
}

pub fn stored_redirect_or(cookies: &CookieJar, fallback: Origin) -> Redirect {
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

	pub fn login(user: User, cookies: &CookieJar) {
		let session = Session::new(user);
		let session_str = serde_urlencoded::to_string(session).unwrap();
		let session_cookie = Cookie::new(SESSION_COOKIE, session_str);
		cookies.add_private(session_cookie);
	}

	pub fn destroy(cookies: &CookieJar) {
		cookies.remove_private(Cookie::named(SESSION_COOKIE))
	}

	pub async fn user(&self, db: &DbConn) -> Result<User> {
		if Local::now() > self.expiry {
			Err(ZauthError::expired())
		} else {
			User::find(self.user_id, db).await
		}
	}
}

impl FromStr for Session {
	type Err = serde_urlencoded::de::Error;

	fn from_str(cookie: &str) -> std::result::Result<Session, Self::Err> {
		serde_urlencoded::from_str(cookie)
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Session {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserSession {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
		let session = try_outcome!(request.guard::<Session>().await);
		let db =
			try_outcome!(request.guard::<DbConn>().await.map_failure(|_| {
				(Status::InternalServerError, "could not connect to database")
			}));
		match session.user(&db).await {
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminSession {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
		let session = try_outcome!(request.guard::<UserSession>().await);
		let user: User = session.user;
		if user.admin {
			Outcome::Success(AdminSession { admin: user })
		} else {
			Outcome::Failure((Status::Forbidden, "user is not an admin"))
		}
	}
}
