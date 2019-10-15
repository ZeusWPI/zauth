#![feature(decl_macro, proc_macro_hygiene)]
#![recursion_limit = "26"]

extern crate bcrypt;
extern crate chrono;
extern crate rand;
extern crate regex;

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
extern crate maplit;

pub mod controllers;
pub mod ephemeral;
pub mod http_authentication;
pub mod models;
pub mod token_store;

use controllers::*;
use models::user::*;
use rocket::config::Config;
use rocket::Request;
use rocket::Rocket;
use rocket_contrib::serve::StaticFiles;
use rocket_contrib::templates::Template;
use token_store::TokenStore;

use diesel::MysqlConnection;
use diesel::SqliteConnection;
use rocket::fairing::AdHoc;

// Embed diesel migrations (provides embedded_migrations::run())
embed_migrations!();

//#[database("sqlite_database")]
// pub struct DbConn(SqliteConnection);
#[database("mysql_database")]
pub struct DbConn(MysqlConnection);
pub type ConcreteConnection = MysqlConnection;

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
				users_controller::current_user,
				users_controller::create_user,
				users_controller::list_users,
				clients_controller::create_client,
				clients_controller::list_clients,
				sessions_controller::new_session,
				sessions_controller::create_session,
				oauth_controller::authorize,
				oauth_controller::login_get,
				oauth_controller::login_post,
				oauth_controller::grant_get,
				oauth_controller::grant_post,
				oauth_controller::token
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
		.attach(AdHoc::on_attach("Admin user", |rocket| {
			let conn = DbConn::get_one(&rocket).expect("database connection");
			if User::find_and_authenticate("admin", "admin", &conn).is_none() {
				let mut user = User::create(
					NewUser {
						username: String::from("admin"),
						password: String::from("admin"),
					},
					&conn,
				)
				.expect("create admin user");
				user.admin = true;
				user.update(&conn).expect("update admin user");
			}
			Ok(rocket)
		}))
}
