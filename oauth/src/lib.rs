#![feature(decl_macro, proc_macro_hygiene)]

extern crate chrono;
extern crate rand;
extern crate regex;
extern crate rocket_http_authentication;

#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod models;
mod token_store;
mod util;

pub mod oauth;
