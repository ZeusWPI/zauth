#![feature(decl_macro, proc_macro_hygiene, trace_macros)]
#![recursion_limit = "256"]

extern crate chrono;
extern crate lettre;
extern crate pwhash;
extern crate rand;
extern crate regex;
extern crate simple_logger;
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
extern crate log;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

#[macro_use]
pub mod views;
pub mod config;
pub mod controllers;
pub mod db_seed;
pub mod ephemeral;
pub mod errors;
pub mod http_authentication;
pub mod mailer;
pub mod models;
pub mod token_store;
pub mod util;

use diesel_migrations::MigrationHarness;
use lettre::message::Mailbox;
use rocket::fairing::AdHoc;
use rocket::figment::Figment;
use rocket::fs::FileServer;
use rocket::{Build, Rocket};
use rocket_sync_db_pools::database;
use rocket_sync_db_pools::diesel::PgConnection;
use simple_logger::SimpleLogger;

use crate::config::{AdminEmail, Config};
use crate::controllers::*;
use crate::db_seed::Seeder;
use crate::errors::{
	internal_server_error, not_found, not_implemented, unauthorized,
};
use crate::mailer::Mailer;
use crate::token_store::TokenStore;

use std::str::FromStr;

#[database("postgresql_database")]
pub struct DbConn(PgConnection);
pub type ConcreteConnection = PgConnection;

pub const ZAUTH_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations =
	embed_migrations!("migrations");

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
	""
}

pub fn prepare_custom(config: Figment) -> Rocket<Build> {
	assemble(rocket::custom(config))
}

pub fn prepare() -> Rocket<Build> {
	SimpleLogger::new().env().init().unwrap();
	assemble(rocket::build())
}

/// Setup of the given rocket instance. Mount routes, add managed state, and
/// attach fairings.
fn assemble(rocket: Rocket<Build>) -> Rocket<Build> {
	let config: Config = rocket.figment().extract().expect("config");
	let admin_email: AdminEmail = AdminEmail(
		Mailbox::from_str(&config.admin_email).expect("admin email"),
	);
	let token_store = TokenStore::<oauth_controller::UserToken>::new(&config);
	let mailer = Mailer::new(&config).unwrap();

	let rocket = rocket
		.mount(
			"/",
			routes![
				favicon,
				clients_controller::list_clients,
				clients_controller::update_client_page,
				clients_controller::update_client,
				clients_controller::create_client,
				clients_controller::delete_client,
				clients_controller::get_generate_secret,
				clients_controller::post_generate_secret,
				oauth_controller::authorize,
				oauth_controller::do_authorize,
				oauth_controller::grant_get,
				oauth_controller::grant_post,
				oauth_controller::token,
				pages_controller::home_page,
				sessions_controller::create_session,
				sessions_controller::new_session,
				sessions_controller::delete_session,
				sessions_controller::destroy_session,
				users_controller::create_user_page,
				users_controller::create_user,
				users_controller::register_page,
				users_controller::register,
				users_controller::current_user,
				users_controller::show_user,
				users_controller::list_users,
				users_controller::update_user,
				users_controller::change_state,
				users_controller::set_admin,
				users_controller::set_approved,
				users_controller::forgot_password_get,
				users_controller::forgot_password_post,
				users_controller::reset_password_get,
				users_controller::reset_password_post,
				users_controller::confirm_email_get,
				users_controller::confirm_email_post,
				users_controller::show_confirm_unsubscribe,
				users_controller::unsubscribe_user,
				mailing_list_controller::list_mails,
				mailing_list_controller::send_mail,
				mailing_list_controller::show_create_mail_page,
				mailing_list_controller::show_mail,
			],
		)
		.register(
			"/",
			catchers![
				unauthorized,
				not_found,
				internal_server_error,
				not_implemented
			],
		)
		.mount("/static/", FileServer::from("static/"))
		.manage(token_store)
		.manage(mailer)
		.manage(admin_email)
		.attach(DbConn::fairing())
		.attach(AdHoc::config::<Config>())
		.attach(AdHoc::on_ignite("Database preparation", prepare_database));

	// if rocket.config().environment.is_dev() {
	// rocket = util::seed_database(rocket, config);
	//}

	rocket
}

async fn prepare_database(rocket: Rocket<Build>) -> Rocket<Build> {
	eprintln!("Requesting a database connection.");
	let db = DbConn::get_one(&rocket).await.expect("database connection");
	eprintln!("Running migrations.");
	db.run(|conn| {
		conn.run_pending_migrations(MIGRATIONS)
			.expect("diesel migrations");
	})
	.await;

	if rocket.figment().profile() == "debug" {
		eprintln!("Seeding database.");
		let config: Config = rocket.figment().extract().expect("config");
		let seeder = Seeder::from_env();
		seeder
			.run(config.bcrypt_cost, &db)
			.await
			.expect("database seed");
	}

	rocket
}
