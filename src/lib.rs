#![feature(decl_macro, proc_macro_hygiene)]
#![recursion_limit = "256"]

extern crate chrono;
extern crate pwhash;
extern crate rand;
extern crate regex;

#[macro_use]
extern crate error_chain;
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
pub mod errors;
pub mod http_authentication;
pub mod models;
pub mod token_store;

use controllers::*;
use rocket::config::Config;
use rocket::Rocket;
use rocket_contrib::serve::StaticFiles;
use rocket_contrib::templates::Template;
use token_store::TokenStore;

use diesel::MysqlConnection;
use rocket::fairing::AdHoc;

// Embed diesel migrations (provides embedded_migrations::run())
embed_migrations!();

#[database("mysql_database")]
pub struct DbConn(MysqlConnection);
pub type ConcreteConnection = MysqlConnection;

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
	""
}

pub fn prepare_custom(config: Config) -> Rocket {
	assemble(rocket::custom(config))
}

pub fn prepare() -> Rocket {
	assemble(rocket::ignite())
}

/// Setup of the given rocket instance. Mount routes, add managed state, and
/// attach fairings.
fn assemble(rocket: Rocket) -> Rocket {
	rocket
		.mount(
			"/",
			routes![
				favicon,
				clients_controller::create_client,
				clients_controller::list_clients,
				oauth_controller::authorize,
				oauth_controller::grant_get,
				oauth_controller::grant_post,
				oauth_controller::login_get,
				oauth_controller::login_post,
				oauth_controller::token,
				pages_controller::home_page,
				sessions_controller::create_session,
				sessions_controller::new_session,
				sessions_controller::delete_session,
				sessions_controller::destroy_session,
				users_controller::create_user,
				users_controller::current_user,
				users_controller::list_users,
			],
		)
		.mount("/static/", StaticFiles::from("static/"))
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
