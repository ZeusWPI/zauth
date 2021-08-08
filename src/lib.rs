#![feature(decl_macro, proc_macro_hygiene, trace_macros)]
#![recursion_limit = "256"]

extern crate chrono;
extern crate lettre;
extern crate pwhash;
extern crate rand;
extern crate regex;
extern crate thiserror;
extern crate toml;
extern crate validator;

#[macro_use]
extern crate rocket;
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate validator_derive;

#[macro_use]
pub mod views;
pub mod config;
pub mod controllers;
pub mod ephemeral;
pub mod errors;
pub mod http_authentication;
pub mod mailer;
pub mod models;
pub mod token_store;
pub mod util;
// pub mod db_seed;

use crate::config::Config;
use crate::controllers::*;
use crate::token_store::TokenStore;
use rocket::{Build, Rocket};

use crate::mailer::Mailer;
use rocket::fairing::AdHoc;
use rocket::fs::FileServer;

use rocket_sync_db_pools::database;
use rocket_sync_db_pools::diesel::PgConnection;

#[database("postgresql_database")]
pub struct DbConn(PgConnection);
pub type ConcreteConnection = PgConnection;

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
	""
}

pub fn prepare_custom(config: rocket::Config) -> Rocket<Build> {
	assemble(rocket::custom(config))
}

pub fn prepare() -> Rocket<Build> {
	assemble(rocket::build())
}

/// Setup of the given rocket instance. Mount routes, add managed state, and
/// attach fairings.
fn assemble(rocket: Rocket<Build>) -> Rocket<Build> {
	let config: Config = rocket.figment().extract().expect("config");
	let token_store = TokenStore::<oauth_controller::UserToken>::new(&config);
	let mailer = Mailer::new(&config).unwrap();

	let rocket = rocket
		.mount(
			"/",
			routes![
				favicon,
				/* clients_controller::list_clients,
				 * clients_controller::update_client_page,
				 * clients_controller::update_client,
				 * clients_controller::create_client,
				 * clients_controller::delete_client,
				 * oauth_controller::authorize,
				 * oauth_controller::grant_get,
				 * oauth_controller::grant_post,
				 * oauth_controller::token,
				 * pages_controller::home_page,
				 * sessions_controller::create_session,
				 * sessions_controller::new_session,
				 * sessions_controller::delete_session,
				 * sessions_controller::destroy_session,
				 * users_controller::create_user_page,
				 * users_controller::create_user,
				 * users_controller::register_page,
				 * users_controller::register,
				 * users_controller::current_user,
				 * users_controller::show_user,
				 * users_controller::list_users,
				 * users_controller::update_user,
				 * users_controller::set_admin,
				 * users_controller::set_approved,
				 * users_controller::forgot_password_get,
				 * users_controller::forgot_password_post,
				 * users_controller::reset_password_get,
				 * users_controller::reset_password_post, */
			],
		)
		.mount("/static/", FileServer::from("static/"))
		.manage(token_store)
		.manage(mailer)
		.manage(config.clone())
		.attach(DbConn::fairing())
		.attach(AdHoc::config::<Config>())
		.attach(AdHoc::on_ignite("Database Migrations", run_migrations));

	// if rocket.config().environment.is_dev() {
	// rocket = util::seed_database(rocket, config);
	//}

	rocket
}

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
	// This macro from `diesel_migrations` defines an `embedded_migrations`
	// module containing a function named `run` that runs the migrations in the
	// specified directory, initializing the database.
	embed_migrations!("migrations");

	let db = DbConn::get_one(&rocket).await.expect("database connection");
	db.run(|conn| embedded_migrations::run(conn))
		.await
		.expect("diesel migrations");

	rocket
}
