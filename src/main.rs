#![feature(decl_macro, proc_macro_hygiene)]

extern crate rocket_contrib;
extern crate chrono;
extern crate regex;
extern crate rand;

#[macro_use] extern crate rocket;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lazy_static;

mod oauth;
mod models;
mod token_store;
mod http_authentication;

use rocket::Rocket;
use oauth::{UserProvider, ClientProvider};

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
    ""
}

struct UserProviderImpl {}

impl UserProvider for UserProviderImpl {
    fn authorize_user(&self, user_id : &str, user_password : &str) -> bool {
        true
    }
    fn user_access_token(&self, user_id : &str) -> String {
        format!("This is an access token for {}", user_id)
    }
}

struct ClientProviderImpl {}

impl ClientProvider for ClientProviderImpl {
    fn client_exists(&self, client_id : &str) -> bool {
        true
    }
    fn client_has_uri(&self, client_id : &str,  redirect_uri : &str) -> bool {
        true
    }
    fn authorize_client(&self, client_id : &str, client_password : &str) -> bool {
        true
    }
}

fn rocket() -> Rocket {
    let rocket = rocket::ignite();
    let cp = ClientProviderImpl {};
    let up = UserProviderImpl {};
    oauth::mount("/oauth/", rocket, cp, up)
        .mount("/", routes![favicon])
}


fn main() {
    rocket().launch();
}

