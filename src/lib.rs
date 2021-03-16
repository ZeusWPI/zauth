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

use crate::config::Config;
use crate::controllers::*;
use crate::models::user::*;
use crate::token_store::TokenStore;
use rocket::Rocket;
use rocket_contrib::serve::StaticFiles;

use crate::mailer::Mailer;
use diesel::PgConnection;
use rocket::fairing::AdHoc;
use std::convert::TryFrom;

// Embed diesel migrations (provides embedded_migrations::run())
embed_migrations!();

#[database("postgresql_database")]
pub struct DbConn(PgConnection);
pub type ConcreteConnection = PgConnection;

#[get("/favicon.ico")]
pub fn favicon() -> &'static str {
	""
}

pub fn prepare_custom(config: rocket::Config) -> Rocket {
	assemble(rocket::custom(config))
}

pub fn prepare() -> Rocket {
	assemble(rocket::ignite())
}

/// Setup of the given rocket instance. Mount routes, add managed state, and
/// attach fairings.
fn assemble(rocket: Rocket) -> Rocket {
	let rocket_config = rocket.config();
	let config: Config = Config::try_from(rocket_config).unwrap();
	let token_store = TokenStore::<oauth_controller::UserToken>::new(&config);
	let mailer = Mailer::new(&config).unwrap();

	let mut rocket = rocket
		.mount(
			"/",
			routes![
				favicon,
				clients_controller::create_client,
				clients_controller::list_clients,
				oauth_controller::authorize,
				oauth_controller::grant_get,
				oauth_controller::grant_post,
				oauth_controller::token,
				pages_controller::home_page,
				sessions_controller::create_session,
				sessions_controller::new_session,
				sessions_controller::delete_session,
				sessions_controller::destroy_session,
				users_controller::create_user,
				users_controller::register_page,
				users_controller::register,
				users_controller::current_user,
				users_controller::show_user,
				users_controller::list_users,
				users_controller::update_user,
				users_controller::set_admin,
				users_controller::forgot_password_get,
				users_controller::forgot_password_post,
				users_controller::reset_password_get,
				users_controller::reset_password_post,
			],
		)
		.mount("/static/", StaticFiles::from("static/"))
		.manage(token_store)
		.manage(mailer)
		.manage(config.clone())
		.attach(DbConn::fairing())
		.attach(AdHoc::on_attach("Database Migrations", run_migrations));

	if rocket.config().environment.is_dev() {
		if let Ok(pw) = std::env::var("ZAUTH_ADMIN_PASSWORD") {
			rocket = rocket
				.attach(AdHoc::on_attach("Create admin user", |rocket| {
					create_admin(rocket, config, pw)
				}));
		}
	}

	rocket
}

fn run_migrations(rocket: Rocket) -> std::result::Result<Rocket, Rocket> {
	let conn = DbConn::get_one(&rocket).expect("database connection");
	match embedded_migrations::run(&*conn) {
		Ok(()) => Ok(rocket),
		Err(e) => {
			eprintln!("Failed to run database migrations: {:?}", e);
			Err(rocket)
		},
	}
}

fn create_admin(
	rocket: Rocket,
	config: Config,
	password: String,
) -> std::result::Result<Rocket, Rocket> {
	let username = String::from("admin");
	let conn = DbConn::get_one(&rocket).expect("database connection");
	let admin = User::find_by_username(&username, &conn)
		.or_else(|_e| {
			User::create(
				NewUser {
					username:  username.clone(),
					password:  password.clone(),
					full_name: String::from("Admin McAdmin"),
					email:     String::from("admin@example.com"),
					ssh_key:   None,
				},
				config.bcrypt_cost,
				&conn,
			)
		})
		.and_then(|mut user| {
			user.admin = true;
			user.update(&conn)
		});
	match admin {
		Ok(_admin) => {
			println!(
				"Admin created with username \"{}\" and password \"{}\"",
				username, password
			);
			Ok(rocket)
		},
		Err(e) => {
			eprintln!("Error creating admin {:?}", e);
			Err(rocket)
		},
	}
}
