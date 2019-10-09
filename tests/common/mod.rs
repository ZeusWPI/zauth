extern crate diesel;
extern crate rocket;
extern crate tempfile;
extern crate urlencoding;
extern crate zauth;

use diesel::prelude::*;
use rocket::config::{Config, Value};
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::local::Client;
use std::collections::HashMap;
use std::str::FromStr;

use common::zauth::models::user::*;

pub fn url(content: &str) -> String {
	urlencoding::encode(content)
}

/// Creates a rocket::local::Client for testing purposes. The rocket instance
/// will be configured with a Sqlite database located in a tmpdir  This
/// executes the given function with the Client a connection to that
/// database. The database and its directory will be removed after the function
/// has run.
pub fn with<F>(run: F)
where F: FnOnce(Client, SqliteConnection) -> () {
	let dir = tempfile::tempdir().unwrap();
	let db_path = dir.path().join("db.sqlite");
	let db_path_str = db_path.to_str().unwrap();

	let mut cfg = HashMap::new();
	cfg.insert("template_dir".into(), "templates".into());

	let cfg_str = format!("sqlite_database = {{ url = \"{}\" }}", db_path_str);
	let databases: Value = Value::from_str(&cfg_str).unwrap();
	cfg.insert("databases".into(), databases);

	let mut config = Config::development();
	config.set_extras(cfg);

	let client = Client::new(zauth::prepare_custom(config))
		.expect("valid rocket instance");

	let db = SqliteConnection::establish(db_path_str).unwrap();

	run(client, db);
}

pub fn with_admin<F>(run: F)
where F: FnOnce(Client, SqliteConnection) -> () {
	with(|client, db| {
		let mut user = User::create(
			NewUser {
				username: String::from("admin"),
				password: String::from("admin"),
			},
			&db,
		)
		.unwrap();
		user.admin = true;
		user.update(&db);

		{
			let response = client
				.post("/login")
				.body("username=admin&password=admin")
				.header(ContentType::Form)
				.dispatch();
			assert_eq!(response.status(), Status::SeeOther);
		}

		run(client, db);
	});
}
