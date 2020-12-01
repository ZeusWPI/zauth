use crate::ephemeral::cookieable::{CookieName, Cookieable, Wrapped};
use crate::ephemeral::session::Session;
use crate::errors::{RequestError, Result, ZauthError};
use crate::util::random_token;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Cookies, Method};
use rocket::request::FromRequest;
use rocket::{Data, Request};

const CSRF_TOKEN_LENGTH: usize = 64;
const CSRF_COOKIE: &'static str = "__Host-CSRF";

#[derive(Serialize, Deserialize)]
pub struct CsrfToken {
	token: String,
}

impl CsrfToken {
	pub fn generate() -> Self {
		CsrfToken {
			token: random_token(CSRF_TOKEN_LENGTH),
		}
	}
}

#[derive(Serialize, Deserialize)]
pub struct CsrfCookie {
	token:   CsrfToken,
	user_id: Option<i32>,
}

impl CookieName for CsrfCookie {
	const COOKIE_NAME: &'static str = CSRF_COOKIE;
}

impl Cookieable for CsrfCookie {}

pub trait CsrfAble {
	fn csrf_token(&self) -> &CsrfToken;
	fn csrf_check(&self, request: &Request) -> Result<()> {
		let session: Option<Wrapped<Session>> = Option::from_request(request)?;
		let user_id = session.map(|s| s.user_id()?);
		let cookie: Wrapped<CsrfCookie> = Wrapped::from_request(request)?;
		if cookie.user_id != user_id || cookie.token != self.csrf_token() {
			Err(ZauthError::Request(RequestError::CSRFError))
		}
		Ok(())
	}
}
