#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate chrono;

#[macro_use] extern crate serde_derive;

mod oauth;
mod models;

use rocket_contrib::Template;


fn main() {
    let rocket = rocket::ignite();
    oauth::mount("/oauth/", rocket)
        .attach(Template::fairing())
        .launch();
}
