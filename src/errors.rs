use rocket::Request;
use rocket::http::Status;
use rocket::response::{self, Responder, Response};
use thiserror::Error;

use diesel::result::Error::NotFound;
use lettre::Message;
use log::warn;
use rocket::serde::json::Json;
use rocket::tokio::sync::mpsc::error::{SendError, TrySendError};
use std::convert::Infallible;
use validator::ValidationErrors;
use webauthn_rs::prelude::WebauthnError;

use crate::views::accepter::Accepter;

#[derive(Error, Debug)]
pub enum ZauthError {
	#[error("Internal server error {0:?}")]
	Internal(#[from] InternalError),
	#[error("Launch error {0:?}")]
	Launch(#[from] LaunchError),
	#[error("Not found: {0:?}")]
	NotFound(String),
	#[error("Unprocessable request: {0:?}")]
	Unprocessable(String),
	#[error("Validation error: {0:?}")]
	ValidationError(#[from] ValidationErrors),
	#[error("OAuth error: {0:?}")]
	OAuth(#[from] OAuthError),
	#[error("Authentication error {0:?}")]
	AuthError(#[from] AuthenticationError),
	#[error("Login error {0:?}")]
	LoginError(#[from] LoginError),
	#[error("Webauthn error: {0:?}")]
	WebauthnError(#[from] WebauthnError),
	#[error("Infallible")]
	Infallible(#[from] Infallible),
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
	error: &'static str,
	status: u16,
	message: Option<String>,
}

impl<'r, 'o: 'r> Responder<'r, 'o> for ZauthError {
	fn respond_to(self, request: &'r Request<'_>) -> response::Result<'o> {
		let mut builder = Response::build();
		let debug = request.rocket().figment().profile() == "debug";
		match self {
			ZauthError::NotFound(_) => {
				builder.status(Status::NotFound);
				builder.merge(not_found().respond_to(request)?);
			},
			ZauthError::Unprocessable(message) => {
				builder.status(Status::UnprocessableEntity);
				builder.merge(
					unprocessable_with_message(Some(message))
						.respond_to(request)?,
				);
			},
			ZauthError::Internal(e) => {
				warn!("Internal error occurred: {:?}", e);
				let message = if debug {
					format!("{:?}", e)
				} else {
					"Check the logs for the actual error.".to_string()
				};
				builder.status(Status::InternalServerError);
				builder.merge(
					internal_server_error_with_message(message)
						.respond_to(request)?,
				);
			},
			ZauthError::AuthError(_) => {
				builder.status(Status::Unauthorized);
				builder.merge(unauthorized().respond_to(request)?);
			},
			_ => {
				warn!("Unmapped error occurred: {:?}", self);
				let message = if debug {
					format!("{:?}", self)
				} else {
					"Check the logs for the actual error.".to_string()
				};
				builder.status(Status::NotImplemented);
				builder.merge(
					not_implemented_with_message(message)
						.respond_to(request)?,
				);
			},
		};

		Ok(builder.finalize())
	}
}

#[catch(401)]
pub fn unauthorized<'r>() -> impl Responder<'r, 'static> {
	Accepter {
		html: template!("errors/401.html"),
		json: Json(JsonError {
			error: "unauthorized",
			status: 401,
			message: None,
		}),
	}
}

#[catch(404)]
pub fn not_found<'r>() -> impl Responder<'r, 'static> {
	Accepter {
		html: template!("errors/404.html"),
		json: Json(JsonError {
			error: "not found",
			status: 404,
			message: None,
		}),
	}
}

#[catch(422)]
pub fn unprocessable<'r>() -> impl Responder<'r, 'static> {
	unprocessable_with_message(None)
}

pub fn unprocessable_with_message<'r>(
	message: Option<String>,
) -> impl Responder<'r, 'static> {
	Accepter {
		html: template!("errors/422.html";
			message: Option<String> = message.clone()
		),
		json: Json(JsonError {
			error: "unprocessable",
			status: 422,
			message,
		}),
	}
}

#[catch(500)]
pub fn internal_server_error<'r>() -> impl Responder<'r, 'static> {
	internal_server_error_with_message("Internal rocket error".to_string())
}

fn internal_server_error_with_message<'r>(
	message: String,
) -> impl Responder<'r, 'static> {
	Accepter {
		html: template!("errors/500.html"; error: String = message.clone()),
		json: Json(JsonError {
			error: "internal server error",
			status: 500,
			message: Some(message),
		}),
	}
}

#[catch(501)]
pub fn not_implemented<'r>() -> impl Responder<'r, 'static> {
	not_implemented_with_message("Rocket not implemented error".to_string())
}

fn not_implemented_with_message<'r>(
	message: String,
) -> impl Responder<'r, 'static> {
	Accepter {
		html: template!("errors/501.html"; error: String = message.clone()),
		json: Json(JsonError {
			error: "not implemented",
			status: 501,
			message: Some(message),
		}),
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
	#[error("Bincode encoding error")]
	BincodeEncodeError(#[from] bincode::error::EncodeError),
	#[error("Bincode decoding error")]
	BincodeDecodeError(#[from] bincode::error::DecodeError),
	#[error("B64 decode error")]
	Base64DecodeError(#[from] base64::DecodeError),
	#[error("JWT error")]
	JWTError(#[from] jsonwebtoken::errors::Error),
	#[error("Serde error")]
	SerdeError(#[from] serde_json::Error),
}

pub type InternalResult<T> = std::result::Result<T, InternalError>;

#[derive(Error, Debug)]
pub enum LoginError {
	#[error("Username or password incorrect")]
	UsernamePasswordError,
	#[error("Admin approval pending for this account")]
	AccountPendingApprovalError,
	#[error("Email confirmation pending for this account")]
	AccountPendingMailConfirmationError,
	#[error("Account disabled")]
	AccountDisabledError,
	#[error("Passkey authentication failed")]
	PasskeyError,
	#[error(
		"Passkey authentication failed, if registered key is non-resident \
		 make sure to supply a username"
	)]
	PasskeyDiscoverableError,
}

#[derive(Error, Debug)]
pub enum AuthenticationError {
	#[error("Not authorized '{0}'")]
	Unauthorized(String),
	#[error("Authentication failed")]
	AuthFailed,
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
	#[error("Only grant_type=authorization_code is supported")]
	GrantTypeMismatch,
	#[error("Invalid request")]
	InvalidRequest,
	#[error("Invalid grant: {0}")]
	InvalidGrant(String),
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
