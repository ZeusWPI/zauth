#![feature(decl_macro, proc_macro_hygiene, trace_macros)]
#![recursion_limit = "256"]

extern crate chrono;
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
pub mod models;
pub mod token_store;

use crate::controllers::*;
use crate::token_store::TokenStore;
use rocket::config::Config;
use rocket::Rocket;
use rocket_contrib::serve::StaticFiles;

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
				}
			}
		}))
}

use rocket::response::Responder;
use rocket::Request;
pub enum Either<R, E> {
	Left(R),
	Right(E),
}

impl<'r, R, E> Responder<'r> for Either<R, E>
where
	R: Responder<'r>,
	E: Responder<'r>,
{
	fn respond_to(self, req: &Request) -> rocket::response::Result<'r> {
		match self {
			Self::Left(left) => left.respond_to(req),
			Self::Right(right) => right.respond_to(req),
		}
	}
}
