use rocket::http::Status;
use rocket::response::{self, Responder, Response};
use rocket::Request;
use thiserror::Error;

use diesel::result::Error::NotFound;
use lettre::Message;
use rocket::serde::json::Json;
use std::sync::mpsc::{SendError, TrySendError};
use validator::ValidationErrors;

use crate::views::accepter::Accepter;

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
	#[error("OAuth error: {0:?}")]
	OAuth(#[from] OAuthError),
	#[error("Authentication error {0:?}")]
	AuthError(#[from] AuthenticationError),
	#[error("Login error {0:?}")]
	LoginError(#[from] LoginError),
}
impl ZauthError {
	pub fn not_found(what: &str) -> Self {
		ZauthError::NotFound(what.to_string())
	}

	pub fn expired() -> Self {
		Self::AuthError(AuthenticationError::SessionExpired)
	}
}

#[derive(Serialize)]
struct JsonError {
	error:   &'static str,
	status:  u16,
	message: Option<String>,
}

impl<'r, 'o: 'r> Responder<'r, 'o> for ZauthError {
	fn respond_to(self, request: &'r Request<'_>) -> response::Result<'o> {
		let mut builder = Response::build();
		match self {
			ZauthError::NotFound(_) => {
				builder.status(Status::NotFound);
				builder.merge(
					Accepter {
						html: template!("errors/404.html"),
						json: Json(JsonError {
							error:   "not found".into(),
							status:  404,
							message: None,
						}),
					}
					.respond_to(request)?,
				);
			},
			ZauthError::Internal(e) => {
				builder.status(Status::InternalServerError);
				builder.merge(
					Accepter {
						html: template!("errors/500.html"; error: String = format!("{:?}", e)),
						json: Json(JsonError {
							error:   "internal server error",
							status:  500,
							message: Some(format!("{:?}", e)),
						}),
					}
					.respond_to(request)?,
				);
			},
			ZauthError::AuthError(_) => {
				builder.status(Status::Unauthorized);
				builder.merge(
					Accepter {
						html: template!("errors/401.html"),
						json: Json(JsonError {
							error:   "unauthorized",
							status:  401,
							message: None,
						}),
					}
					.respond_to(request)?,
				);
			},
			_ => {
				builder.status(Status::NotImplemented);
				builder.merge(
					Accepter {
						html: template!("errors/501.html"; error: String = format!("{:?}", self)),
						json: Json(JsonError {
							error:   "not implemented",
							status:  501,
							message: Some(format!("{:?}", self)),
						}),
					}
					.respond_to(request)?,
				);
			},
		};

		Ok(builder.finalize())
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
	#[error("Admin approval pending for this accountl")]
	AccountPendingApprovalError,
	#[error("Email confirmation pending for this account")]
	AccountPendingMailConfirmationError,
	#[error("Account disabled")]
	AccountDisabledError,
}

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
	#[error("Failed to create SMTP transport: '{0}'")]
	SMTPError(#[from] lettre::transport::smtp::Error),
}

#[derive(Error, Debug)]
pub enum OAuthError {
	#[error(
		"The cookie used for storing OAuth information is invalid or has \
		 expired."
	)]
	InvalidCookie,
	#[error("Only response_type=code is supported")]
	ResponseTypeMismatch,
	#[error("Invalid request")]
	InvalidRequest,
}

pub enum Either<R, E> {
	Left(R),
	Right(E),
}

impl<'r, 'o: 'r, 'a: 'o, 'b: 'o, A, B> Responder<'r, 'r> for Either<A, B>
where
	A: Responder<'r, 'a>,
	B: Responder<'r, 'b>,
{
	fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'o> {
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

impl<'r, 'o: 'r, 'x: 'o, 'y: 'o, 'z: 'o, X, Y, Z> Responder<'r, 'o>
	for OneOf<X, Y, Z>
where
	X: Responder<'r, 'x>,
	Y: Responder<'r, 'y>,
	Z: Responder<'r, 'z>,
{
	fn respond_to(self, req: &'r Request<'_>) -> rocket::response::Result<'o> {
		match self {
			Self::One(one) => one.respond_to(req),
			Self::Two(two) => two.respond_to(req),
			Self::Three(three) => three.respond_to(req),
		}
	}
}
