use rocket::http::Status;
use rocket::response::{self, Responder, Response};
use rocket::Request;
use thiserror::Error;

use diesel::result::Error::NotFound;
use lettre::Message;
use std::io::Cursor;
use std::sync::mpsc::{SendError, TrySendError};
use validator::ValidationErrors;

#[derive(Error, Debug)]
pub enum ZauthError {
	#[error("Internal server error {0:?}")]
	Internal(#[from] InternalError),
	#[error("Launch error {0:?}")]
	Launch(#[from] LaunchError),
	#[error("Not found: {0:?}")]
	NotFound(String),
	#[error("Validation error: {0:?}")]
	ValidationError(#[from] ValidationErrors),
	#[error("Request error {0:?}")]
	RequestError(#[from] RequestError),
	#[error("OAuth error: {0:?}")]
	OAuth(#[from] OAuthError),
	#[error("Authentication error {0:?}")]
	AuthError(#[from] AuthenticationError),
	#[error("Login error {0:?}")]
	LoginError(#[from] LoginError),
	#[error("{0}")]
	Custom(Status, String),
}
impl ZauthError {
	pub fn not_found(what: &str) -> Self {
		ZauthError::NotFound(what.to_string())
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
			ZauthError::NotFound(_) => {
				builder.status(Status::NotFound);
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
		match error {
			NotFound => ZauthError::not_found(&error.to_string()),
			_ => ZauthError::Internal(InternalError::DatabaseError(error)),
		}
	}
}
pub type Result<T> = std::result::Result<T, ZauthError>;

#[derive(Error, Debug)]
pub enum InternalError {
	#[error("Hash error")]
	HashError(#[from] pwhash::error::Error),
	#[error("Database error")]
	DatabaseError(#[from] diesel::result::Error),
	#[error("Template error")]
	TemplateError(#[from] askama::Error),
	#[error("Invalid email: {0}")]
	InvalidEmail(#[from] lettre::address::AddressError),
	#[error("Mailer error")]
	MailError(#[from] lettre::error::Error),
	#[error("Mailer stopped processing items")]
	MailerStopped(#[from] SendError<Message>),
	#[error("Mail queue full")]
	MailQueueFull(#[from] TrySendError<Message>),
	#[error("Bincode error")]
	BincodeError(#[from] Box<bincode::ErrorKind>),
	#[error("B64 decode error")]
	Base64DecodeError(#[from] base64::DecodeError),
}
pub type InternalResult<T> = std::result::Result<T, InternalError>;

#[derive(Error, Debug)]
pub enum LoginError {
	#[error("Username or password incorrect")]
	UsernamePasswordError,
	#[error("Account still pending for admin approval")]
	AccountPendingApprovalError,
	#[error("Account pending for mail approval")]
	AccountPendingMailConfirmationError,
	#[error("Account disabled")]
	AccountDisabledError,
}
pub type LoginResult<T> = std::result::Result<T, LoginError>;

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

#[derive(Error, Debug)]
pub enum LaunchError {
	#[error("Incorrect config value type for key '{0}'")]
	BadConfigValueType(String),
	#[error("Incorrect email address '{0}'")]
	InvalidEmail(#[from] lettre::address::AddressError),
	#[error("Failed to create SMTP transport")]
	SMTPError(#[from] lettre::transport::smtp::error::Error),
}

#[derive(Error, Debug)]
pub enum OAuthError {
	#[error(
		"The cookie used for storing OAuth information is invalid or has \
		 expired."
	)]
	InvalidCookie,
}

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

pub enum OneOf<X, Y, Z> {
	One(X),
	Two(Y),
	Three(Z),
}

impl<'r, X, Y, Z> Responder<'r> for OneOf<X, Y, Z>
where
	X: Responder<'r>,
	Y: Responder<'r>,
	Z: Responder<'r>,
{
	fn respond_to(self, req: &Request) -> rocket::response::Result<'r> {
		match self {
			Self::One(one) => one.respond_to(req),
			Self::Two(two) => two.respond_to(req),
			Self::Three(three) => three.respond_to(req),
		}
	}
}
