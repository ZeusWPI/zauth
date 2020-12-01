use chrono::{DateTime, Duration, Local};
use rocket::http::{Cookie, Cookies, Status};
use rocket::request::{FromRequest, Outcome, Request};

use crate::controllers::sessions_controller::rocket_uri_macro_new_session;
use crate::ephemeral::cookieable::{CookieName, Cookieable, Wrapped};
use crate::errors::{InternalError, Result, UnavailableError, ZauthError};
use crate::models::user::User;
use crate::DbConn;
use rand::{thread_rng, Rng};
use rocket::http::uri::Origin;
use rocket::response::Redirect;
use std::ops::Deref;

pub const SESSION_VALIDITY_MINUTES: i64 = 59;
const REDIRECT_COOKIE: &str = "__Host-Redirect";
const SESSION_COOKIE: &str = "__Host-Session";

pub fn ensure_logged_in_and_redirect(
	mut cookies: Cookies,
	uri: Origin,
) -> Redirect {
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
	session_nonce: usize,
	user_id:       i32,
	expiry:        DateTime<Local>,
}

impl Session {
	pub fn new(user: User) -> Session {
		Session {
			session_nonce: thread_rng().gen(),
			user_id:       user.id,
			expiry:        Local::now()
				+ Duration::minutes(SESSION_VALIDITY_MINUTES),
		}
	}

	pub fn login(user: User, cookies: &mut Cookies) -> Result<()> {
		let session = Session::new(user);
		session
			.into_cookies(cookies)
			.map_err(ZauthError::cookie_error)
	}

	pub fn destroy(cookies: &mut Cookies) {
		cookies.remove_private(Cookie::named(SESSION_COOKIE))
	}

	pub fn user_id(&self) -> Result<i32> {
		if Local::now() > self.expiry {
			Err(ZauthError::expired())
		}
		Ok(self.user_id)
	}

	pub fn user(&self, conn: &DbConn) -> Result<User> {
		User::find(self.user_id()?, conn)
	}
}

impl CookieName for Session {
	const COOKIE_NAME: &'static str = SESSION_COOKIE;
}

impl Cookieable for Session {}

#[derive(Debug)]
pub struct UserSession {
	pub user: User,
	session:  Session,
}

impl<'a, 'r> FromRequest<'a, 'r> for UserSession {
	type Error = ZauthError;

	fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
		let session: Session = request.guard::<Wrapped<Session>>()?.0;
		let db = request.guard::<DbConn>().map_failure(|_| {
			(
				Status::ServiceUnavailable,
				ZauthError::Unavailable(UnavailableError::DatabaseUnavailable),
			)
		})?;
		match session.user(&db) {
			Ok(user) => Outcome::Success(UserSession { user, session }),
			_ => Outcome::Failure((
				Status::InternalServerError,
				ZauthError::Internal(InternalError::InvalidSession(
					"unknown user id in session",
				)),
			)),
		}
	}
}

#[derive(Debug)]
pub struct AdminSession {
	pub admin: User,
	session:   Session,
}

impl<'a, 'r> FromRequest<'a, 'r> for AdminSession {
	type Error = ZauthError;

	fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
		let user_session = request.guard::<UserSession>()?;
		let user: User = user_session.user;
		let session: Session = user_session.session;
		if user.admin {
			Outcome::Success(AdminSession {
				admin: user,
				session,
			})
		} else {
			Outcome::Failure((
				Status::Forbidden,
				ZauthError::Forbidden("not an admin"),
			))
		}
	}
}
