use self::regex::Regex;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::Outcome;

extern crate base64;
extern crate regex;

fn b64_to_credentials(b64: String) -> Option<BasicAuthentication> {
    let credentials = base64::decode(&b64)
        .ok()
        .map_or(None, |bytes| String::from_utf8(bytes).ok())
        .map_or(Vec::new(), |utf8| {
            utf8.split(':').map(|s| s.to_owned()).collect()
        });
    if credentials.len() == 2 {
        Some(BasicAuthentication {
            username: credentials[0].to_string(),
            password: credentials[1].to_string(),
        })
    } else {
        None
    }
}

#[derive(Debug)]
pub struct BasicAuthentication {
    pub username: String,
    pub password: String,
}

impl<'a, 'r> FromRequest<'a, 'r> for BasicAuthentication {
    type Error = String;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<BasicAuthentication, String> {
        let headers: Vec<_> = request.headers().get("Authorization").collect();
        if headers.is_empty() {
            let msg = String::from("Authorization header missing");
            return Outcome::Failure((Status::BadRequest, msg));
        } else if headers.len() > 1 {
            let msg = String::from("More than one authorization header");
            return Outcome::Failure((Status::BadRequest, msg));
        }

        let auth_header = headers[0];
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^Basic ([[[:alnum:]]+/=]+)$").unwrap();
        }

        let b64_credentials = RE.captures(auth_header).map(|c| c[1].to_string());
        if let Some(credentials) = b64_credentials.map_or(None, b64_to_credentials) {
            Outcome::Success(credentials)
        } else {
            let msg = "Unable to parse credentials".to_string();
            Outcome::Failure((Status::BadRequest, msg))
        }
    }
}
