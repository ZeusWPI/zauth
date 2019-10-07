#![feature(decl_macro, proc_macro_hygiene)]
#![recursion_limit = "26"]

extern crate bcrypt;
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

pub mod controllers;
pub mod ephemeral;
pub mod models;
pub mod token_store;

use controllers::*;
use rocket::config::Config;
use rocket::Rocket;
use rocket_contrib::templates::Template;
use token_store::TokenStore;

use diesel::SqliteConnection;
use rocket::fairing::AdHoc;

// Embed diesel migrations (provides embedded_migrations::run())
embed_migrations!();

#[database("sqlite_database")]
pub struct DbConn(SqliteConnection);

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
	""
}

pub fn prepare_custom(config: Config) -> Rocket {
	build_rocket(rocket::custom(config))
}

pub fn prepare() -> Rocket {
	build_rocket(rocket::ignite())
}

/// Setup of the given rocket instance. Mount routes, add managed state, and
/// attach fairings.
fn build_rocket(rocket: Rocket) -> Rocket {
	rocket
		.mount(
			"/",
			routes![
				favicon,
				user_controller::current_user,
				user_controller::create_user,
				user_controller::list_users,
				oauth_controller::authorize,
				oauth_controller::login_get,
				oauth_controller::login_post,
				oauth_controller::grant_get,
				oauth_controller::grant_post,
				oauth_controller::token
			],
		)
		.manage(TokenStore::<oauth_controller::UserToken>::new())
		.attach(DbConn::fairing())
		.attach(Template::fairing())
		.attach(AdHoc::on_attach("Database Migrations", |rocket| {
			let conn = DbConn::get_one(&rocket).expect("database connection");
			match embedded_migrations::run(&*conn) {
				Ok(()) => Ok(rocket),
				Err(e) => {
					eprintln!("Failed to run database migrations: {:?}", e);
					Err(rocket)
				},
			}
		}))
}
