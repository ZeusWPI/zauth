#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate chrono;
extern crate regex;
extern crate rand;

#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lazy_static;

mod oauth;
mod models;
mod token_store;
mod http_authentication;

use rocket::Rocket;

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
    ""
}

fn rocket() -> Rocket {
    let rocket = rocket::ignite();
    oauth::mount("/oauth/", rocket)
        .mount("/", routes![favicon])
}


fn main() {
    rocket().launch();
}

