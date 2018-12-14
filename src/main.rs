#![feature(decl_macro, proc_macro_hygiene)]

extern crate chrono;
extern crate rand;
extern crate regex;
extern crate rocket_contrib;

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;

mod http_authentication;
mod models;
mod oauth;
mod token_store;
mod util;

use oauth::{ClientProvider, UserProvider};
use rocket::Rocket;

use self::regex::Regex;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::Outcome;
use rocket_contrib::json::Json;

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
    ""
}

#[derive(Serialize)]
pub struct AuthorizationToken {
    username: String,
}

impl<'a, 'r> FromRequest<'a, 'r> for AuthorizationToken {
    type Error = String;

    fn from_request(request: &'a Request<'r>) -> request::Outcome<AuthorizationToken, String> {
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
            static ref RE: Regex = Regex::new(r"^Bearer ([[[:alnum:]]+/=]+)$").unwrap();
        }

        if let Some(token) = RE.captures(auth_header).map(|c| c[1].to_string()) {
            Outcome::Success(AuthorizationToken { username: token })
        } else {
            let msg = "Unable to parse tokenn".to_string();
            Outcome::Failure((Status::BadRequest, msg))
        }
    }
}

#[get("/current_user")]
pub fn current_user(token: AuthorizationToken) -> Json<AuthorizationToken> {
    Json(token)
}

#[derive(Clone)]
struct UserProviderImpl {}

impl UserProvider for UserProviderImpl {
    fn authorize_user(&self, user_id: &str, user_password: &str) -> bool {
        true
    }
    fn user_access_token(&self, user_id: &str) -> String {
        format!("This is an access token for {}", user_id)
    }
}

#[derive(Clone)]
struct ClientProviderImpl {}

impl ClientProvider for ClientProviderImpl {
    fn client_exists(&self, client_id: &str) -> bool {
        true
    }
    fn client_has_uri(&self, client_id: &str, redirect_uri: &str) -> bool {
        true
    }
    fn client_needs_grant(&self, client_id: &str) -> bool {
        true
    }
    fn authorize_client(&self, client_id: &str, client_password: &str) -> bool {
        true
    }
}

fn rocket() -> Rocket {
    let rocket = rocket::ignite();
    let cp = ClientProviderImpl {};
    let up = UserProviderImpl {};
    oauth::mount("/oauth/", rocket, cp, up).mount("/", routes![favicon, current_user])
}

fn main() {
    rocket().launch();
}
