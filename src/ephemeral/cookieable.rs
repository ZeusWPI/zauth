use crate::ephemeral::cookieable::CookieError::{
	BincodeError, CookieNotFound, DecodeB64Error,
};
use crate::errors::{RequestError, ZauthError};
use rocket::http::{Cookie, Cookies, Status};
use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::ops::Deref;
use thiserror::Error;

pub trait CookieName {
	const COOKIE_NAME: &'static str;
}

pub trait Cookieable: DeserializeOwned + Serialize + CookieName {
	fn from_cookies(cookies: &mut Cookies) -> Result<Self, CookieError> {
		let cookie = cookies
			.get_private(Self::COOKIE_NAME)
			.ok_or(CookieNotFound)?;

		let bytes =
			base64::decode_config(&cookie.value(), base64::URL_SAFE_NO_PAD)
				.map_err(DecodeB64Error)?;
		bincode::deserialize(&bytes).map_err(BincodeError)
	}

	fn into_cookies(self, cookies: &mut Cookies) -> Result<(), CookieError> {
		let bytes = bincode::serialize(&self).map_err(BincodeError)?;
		let value = base64::encode_config(bytes, base64::URL_SAFE_NO_PAD);
		Ok(cookies.add_private(Cookie::new(Self::COOKIE_NAME, value)))
	}
}

pub struct Wrapped<T>(T);

impl<T> Deref for Wrapped<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<'a, 'r, T> FromRequest<'a, 'r> for Wrapped<T>
where T: Cookieable
{
	type Error = ZauthError;

	fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
		match T::from_cookies(&mut request.cookies()) {
			Ok(cookie) => Outcome::Success(Wrapped(cookie)),
			Err(e) => Outcome::Failure((
				Status::UnprocessableEntity,
				ZauthError::Request(RequestError::from(e)),
			)),
		}
	}
}

#[derive(Debug, Error)]
pub enum CookieError {
	#[error("Decoding failed due to a base64 error")]
	DecodeB64Error(#[from] base64::DecodeError),
	#[error("Encoding or decoding a cookie failed due to a bincode error")]
	BincodeError(#[from] bincode::Error),
	#[error("Cookie was not present")]
	CookieNotFound,
}
