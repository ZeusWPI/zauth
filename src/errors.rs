use rocket::http::Status;
use rocket::response::{self, Responder};
use rocket::Request;
use thiserror::Error;

use std::io::Cursor;

#[derive(Error, Debug)]
pub enum ZauthError {
	#[error("Encoding error {0:?}")]
	EncodingError(#[from] EncodingError),
	#[error("database error")]
	DatabaseError(#[from] diesel::result::Error),
	#[error("Authentication error {0:?}")]
	AuthError(#[from] AuthenticationError),
	#[error("invalid header (expected {expected:?}, found {found:?})")]
	InvalidHeader { expected: String, found: String },
	#[error("{0}")]
	Custom(Status, String),
	#[error("Session expired")]
	SessionExpired,
}

impl Responder<'static> for ZauthError {
	fn respond_to(self, _: &Request) -> response::Result<'static> {
		Ok(response::Response::build()
			.sized_body(Cursor::new(format!("An error occured: {:?}", self)))
			.finalize())
	}
}

pub type Result<T> = std::result::Result<T, ZauthError>;

#[derive(Error, Debug)]
pub enum EncodingError {
	#[error("hash error")]
	HashError(#[from] pwhash::error::Error),
	#[error("bindecode error")]
	BinDecodeError(#[from] Box<bincode::ErrorKind>),
	#[error("base64 decode error")]
	DecodeError(#[from] base64::DecodeError),
}
pub type EncodingResult<T> = std::result::Result<T, EncodingError>;

#[derive(Error, Debug)]
pub enum AuthenticationError {
	#[error("Not authorized '{0}'")]
	Unauthorized(String),
	#[error("Authentication failed")]
	AuthFailed,
	#[error("only response_type=code is supported")]
	ResponseTypeMismatch,
	#[error("Invalid grant {0}")]
	InvalidGrant(String),
}
pub type AuthResult<T> = std::result::Result<T, AuthenticationError>;
