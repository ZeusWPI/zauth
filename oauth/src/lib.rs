#![feature(decl_macro, proc_macro_hygiene)]

extern crate chrono;
extern crate rand;
extern crate regex;
extern crate rocket_http_authentication;

extern crate rocket_contrib;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;
extern crate diesel;
extern crate diesel_migrations;
extern crate lazy_static;

mod models;
mod token_store;
mod util;

pub mod oauth;
