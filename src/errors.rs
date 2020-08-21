use rocket::http::Status;
use rocket::response::{self, Responder, Response};
use rocket::Request;
use thiserror::Error;

use lettre::SendableEmail;
use std::io::Cursor;
use std::sync::mpsc::SendError;

#[derive(Error, Debug)]
pub enum ZauthError {
	#[error("Internal server error {0:?}")]
	Internal(#[from] InternalError),
	#[error("Request error {0:?}")]
	RequestError(#[from] RequestError),
	#[error("Authentication error {0:?}")]
	AuthError(#[from] AuthenticationError),
	#[error("{0}")]
	Custom(Status, String),
}
impl ZauthError {
	pub fn not_found(what: &str) -> Self {
		ZauthError::Custom(Status::NotFound, what.to_string())
	}

	pub fn expired() -> Self {
		Self::AuthError(AuthenticationError::SessionExpired)
	}
}

impl Responder<'static> for ZauthError {
	fn respond_to(self, _: &Request) -> response::Result<'static> {
		let mut builder = Response::build();
		match self {
			ZauthError::Custom(status, _) => {
				builder.status(status);
			},
			ZauthError::Internal(_) => {
				builder.status(Status::InternalServerError);
			},
			ZauthError::AuthError(_) => {
				builder.status(Status::Unauthorized);
			},
			_ => {},
		}

		Ok(builder
			.sized_body(Cursor::new(format!("An error occured: {:?}", self)))
			.finalize())
		// Ok(match self {
		// 	ZauthError::Custom(status, reason) => {
		// 		Response::build().status(status)
		// 	}
		// 	_ => Response::build(),
		// }
		// .sized_body(Cursor::new(format!("An error occured: {:?}", self)))
		// .finalize())
	}
}

impl From<diesel::result::Error> for ZauthError {
	fn from(error: diesel::result::Error) -> Self {
		ZauthError::Internal(InternalError::DatabaseError(error))
	}
}
pub type Result<T> = std::result::Result<T, ZauthError>;

#[derive(Error, Debug)]
pub enum InternalError {
	#[error("Hash error")]
	HashError(#[from] pwhash::error::Error),
	#[error("Database error")]
	DatabaseError(#[from] diesel::result::Error),
	#[error("Mailer error")]
	MailError(#[from] lettre_email::error::Error),
	#[error("Mail queue synchronisation error")]
	SyncError(#[from] SendError<SendableEmail>),
}
pub type InternalResult<T> = std::result::Result<T, InternalError>;

#[derive(Error, Debug)]
pub enum RequestError {
	#[error("Bindecode error")]
	BinDecodeError(#[from] Box<bincode::ErrorKind>),
	#[error("Base64 decode error")]
	DecodeError(#[from] base64::DecodeError),
	#[error("Invalid header (expected {expected:?}, found {found:?})")]
	InvalidHeader { expected: String, found: String },
	#[error("Only response_type=code is supported")]
	ResponseTypeMismatch,
	#[error("Invalid request")]
	InvalidRequest,
}
pub type EncodingResult<T> = std::result::Result<T, RequestError>;

#[derive(Error, Debug)]
pub enum AuthenticationError {
	#[error("Not authorized '{0}'")]
	Unauthorized(String),
	#[error("Authentication failed")]
	AuthFailed,
	#[error("Invalid grant {0}")]
	InvalidGrant(String),
	#[error("Session expired")]
	SessionExpired,
}
pub type AuthResult<T> = std::result::Result<T, AuthenticationError>;

pub enum Either<R, E> {
	Left(R),
	Right(E),
}

impl<'r, R, E> Responder<'r> for Either<R, E>
where
	R: Responder<'r>,
	E: Responder<'r>,
{
	fn respond_to(self, req: &Request) -> rocket::response::Result<'r> {
		match self {
			Self::Left(left) => left.respond_to(req),
			Self::Right(right) => right.respond_to(req),
		}
	}
}
