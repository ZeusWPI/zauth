#![allow(dead_code)]

extern crate diesel;
extern crate parking_lot;
extern crate rocket;
extern crate tempfile;
extern crate urlencoding;
extern crate zauth;

use diesel::sql_query;
use diesel::RunQueryDsl;
use parking_lot::Mutex;
use rocket::config::{Config, Value};
use rocket::http::ContentType;
use rocket::http::Status;
use std::collections::HashMap;
use std::str::FromStr;

use crate::common::zauth::models::client::*;
use crate::common::zauth::models::user::*;
use crate::common::zauth::DbConn;

type HttpClient = rocket::local::Client;

// Rocket doesn't support transactional testing yet, so we use a lock to
// serialize tests.
static DB_LOCK: Mutex<()> = Mutex::new(());

pub fn url(content: &str) -> String {
	urlencoding::encode(content)
}

fn reset_db(db: &DbConn) {
	assert!(sql_query("TRUNCATE TABLE users").execute(&db.0).is_ok());
	assert!(sql_query("TRUNCATE TABLE clients").execute(&db.0).is_ok());
}

/// Creates a rocket::local::Client for testing purposes. The rocket instance
/// will be configured with a Sqlite database located in a tmpdir  This
/// executes the given function with the Client a connection to that
/// database. The database and its directory will be removed after the function
/// has run.
pub fn as_visitor<F>(run: F)
where F: FnOnce(HttpClient, DbConn) -> () {
	// Prepare config
	let mut cfg = HashMap::new();
	let db_url = "postgresql://zauth:zauth@localhost/zauth_test";
	let cfg_str = format!("postgresql_database = {{ url = \"{}\" }}", db_url);
	let databases: Value = Value::from_str(&cfg_str).unwrap();
	cfg.insert("databases".into(), databases);
	cfg.insert("template_dir".into(), "src/views/".into());
	let mut config = Config::development();
	config.set_extras(cfg);

	let _lock = DB_LOCK.lock();
	let client =
		HttpClient::new(zauth::prepare_custom(config)).expect("rocket client");

	let db = DbConn::get_one(client.rocket()).expect("database connection");
	reset_db(&db);
	assert_eq!(0, User::all(&db).unwrap().len());
	assert_eq!(0, Client::all(&db).unwrap().len());

	run(client, db);
}

pub fn as_user<F>(run: F)
where F: FnOnce(HttpClient, DbConn, User) -> () {
	as_visitor(|client, db| {
		let user = User::create(
			NewUser {
				username: String::from("user"),
				password: String::from("user"),
			},
			&db,
		)
		.unwrap();

		{
			let response = client
				.post("/login")
				.body("username=user&password=user")
				.header(ContentType::Form)
				.dispatch();
			assert_eq!(response.status(), Status::SeeOther, "login failed");
		}

		run(client, db, user);
	});
}

pub fn as_admin<F>(run: F)
where F: FnOnce(HttpClient, DbConn, User) -> () {
	as_visitor(|client, db| {
		let mut user = User::create(
			NewUser {
				username: String::from("admin"),
				password: String::from("admin"),
			},
			&db,
		)
		.unwrap();

		user.admin = true;
		let user = user.update(&db).unwrap();

		{
			let response = client
				.post("/login")
				.body("username=admin&password=admin")
				.header(ContentType::Form)
				.dispatch();
			assert_eq!(response.status(), Status::SeeOther, "login failed");
		}

		run(client, db, user);
	});
}
