use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};

use rocket::Outcome;
use std::str::FromStr;

#[derive(Debug)]
pub struct BasicAuthentication {
	pub user:     String,
	pub password: String,
}

impl FromStr for BasicAuthentication {
	type Err = String;

	fn from_str(b64: &str) -> Result<Self, Self::Err> {
		base64::decode(b64)
			.map_err(|e| e.to_string())
			.and_then(|bytes| {
				String::from_utf8(bytes).map_err(|e| e.to_string())
			})
			.and_then(|utf8| {
				let parts: Vec<&str> = utf8.split(':').collect();
				if parts.len() == 2 {
					Ok(BasicAuthentication {
						user:     String::from(parts[0]),
						password: String::from(parts[1]),
					})
				} else {
					Err(String::from("only one ':' allowed"))
				}
			})
	}
}

impl<'a, 'r> FromRequest<'a, 'r> for BasicAuthentication {
	type Error = String;

	fn from_request(
		request: &'a Request<'r>,
	) -> request::Outcome<Self, Self::Error> {
		let headers: Vec<_> = request.headers().get("Authorization").collect();
		if headers.is_empty() {
			return Outcome::Failure((
				Status::BadRequest,
				String::from("Authorization header missing"),
			));
		} else if headers.len() > 1 {
			return Outcome::Failure((
				Status::BadRequest,
				String::from("More than one authorization header"),
			));
		}

		let auth_header = headers[0];
		let prefix = "Basic ";
		if !auth_header.starts_with(prefix) {
			return Outcome::Failure((
				Status::BadRequest,
				String::from("We only support Basic Authentication"),
			));
		}
		match BasicAuthentication::from_str(&auth_header[prefix.len()..]) {
			Ok(credentials) => Outcome::Success(credentials),
			Err(error_msg) => Outcome::Failure((Status::BadRequest, error_msg)),
		}
	}
}
