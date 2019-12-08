use rocket::http::Status;
use rocket::request::Request;
use rocket::response::{self, Responder, Response};

error_chain! {
	foreign_links {
		SerdeUrlencode(serde_urlencoded::ser::Error);
	}
	errors {
		NotImplemented(message: String) {
			description("not implemented")
			display("Not implemented: '{}'", message)
		}
		Unauthorized(message: String) {
			description("not authorized")
			display("Not authorized: '{}'", message)
		}
	}
}

impl ErrorKind {
	fn status(&self) -> Status {
		match self {
			ErrorKind::NotImplemented(_) => Status::NotImplemented,
			ErrorKind::Unauthorized(_) => Status::Unauthorized,
			_ => Status::InternalServerError,
		}
	}

	fn default_response<'r>(self, req: &Request) -> response::Result<'r> {
		let message = format!("An error occured! {}", self).respond_to(req)?;
		Response::build_from(message).status(self.status()).ok()
	}
}

impl<'r> Responder<'r> for Error {
	fn respond_to(self, req: &Request) -> response::Result<'r> {
		self.0.default_response(req)
	}
}
