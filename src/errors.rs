use crate::oauth_controller::TokenError;
use rocket::http::Status;
use rocket::response::{self, Responder};
use rocket::Request;
use rocket_contrib::json::Json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZauthError {
	#[error("hash error")]
	HashError(#[from] pwhash::error::Error),
	#[error("bindecode error")]
	BinDecodeError(#[from] Box<bincode::ErrorKind>),
	#[error("base64 decode error")]
	DecodeError(#[from] base64::DecodeError),
	#[error("database error")] // Not used yet
	DatabaseError(#[from] diesel::result::Error),
	#[error("Not authorized '{0}'")]
	Unauthorized(String),
	#[error("Authentication failed")]
	AuthFailed,
	#[error("invalid header (expected {expected:?}, found {found:?})")]
	InvalidHeader { expected: String, found: String },
	#[error("unknown data store error")]
	Unknown(String),
	#[error("Not implemented: '{0}'")]
	NotImplemented(String),
	#[error("{0}")]
	Custom(Status, String),
	#[error("Token error {0}")]
	TokenError(#[from] Json<TokenError>),
	#[error("Anyhow error")]
	Anyhow(#[from] anyhow::Error),
}

impl Responder<'static> for ZauthError {
	fn respond_to(self, _: &Request) -> response::Result<'static> {
		Err(Status::ImATeapot)
	}
}

pub type Result<T> = std::result::Result<T, ZauthError>;
