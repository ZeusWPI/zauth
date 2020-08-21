#![feature(decl_macro, proc_macro_hygiene, trace_macros)]
#![recursion_limit = "256"]

extern crate chrono;
extern crate lettre_email;
extern crate pwhash;
extern crate rand;
extern crate regex;

extern crate thiserror;

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
pub mod views;
pub mod controllers;
pub mod ephemeral;
pub mod errors;
pub mod http_authentication;
pub mod mailer;
pub mod models;
pub mod token_store;

use crate::controllers::*;
use crate::models::user::*;
use crate::token_store::TokenStore;
use rocket::config::Config;
use rocket::Rocket;
use rocket_contrib::serve::StaticFiles;

use diesel::PgConnection;
use rocket::fairing::AdHoc;

// Embed diesel migrations (provides embedded_migrations::run())
embed_migrations!();

#[database("postgresql_database")]
pub struct DbConn(PgConnection);
pub type ConcreteConnection = PgConnection;

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
				users_controller::show_user,
				users_controller::list_users,
				users_controller::update_user,
				users_controller::set_admin,
			],
		)
		.mount("/static/", StaticFiles::from("static/"))
		.manage(TokenStore::<oauth_controller::UserToken>::new())
		.attach(DbConn::fairing())
		.attach(AdHoc::on_attach("Database Migrations", |rocket| {
			let conn = DbConn::get_one(&rocket).expect("database connection");
			match embedded_migrations::run(&*conn) {
				Ok(()) => Ok(rocket),
				Err(e) => {
					eprintln!("Failed to run database migrations: {:?}", e);
					Err(rocket)
				},
			}
		}));
	if rocket.config().environment.is_dev() {
		rocket =
			rocket.attach(AdHoc::on_attach("Create admin user", |rocket| {
				let conn =
					DbConn::get_one(&rocket).expect("database connection");
				if let Ok(pw) = std::env::var("ZAUTH_ADMIN_PASSWORD") {
					let admin = User::find_by_username("admin", &conn)
						.or_else(|_e| {
							User::create(
								NewUser {
									username:   String::from("admin"),
									password:   String::from(&pw),
									first_name: String::from(""),
									last_name:  String::from(""),
									email:      String::from(""),
									ssh_key:    None,
								},
								&conn,
							)
						})
						.and_then(|mut user| {
							user.change_with(UserChange {
								username:   None,
								password:   Some(pw),
								first_name: None,
								last_name:  None,
								email:      None,
								ssh_key:    None,
							})?;
							user.admin = true;
							user.update(&conn)
						});
					match admin {
						Ok(admin) => {
							dbg!(admin);
						},
						Err(e) => {
							eprintln!("Error {:?}", e);
							return Err(rocket);
						},
					}
				}
				Ok(rocket)
			}))
	}
	rocket
}
