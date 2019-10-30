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
use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;

use common::zauth::models::user::*;
use common::zauth::DbConn;

pub fn url(content: &str) -> String {
	urlencoding::encode(content)
}

pub fn db(client: &Client) -> DbConn {
	DbConn::get_one(client.rocket()).expect("database connection")
}

pub fn reset_db(db_url: &str) -> () {
	let status = Command::new("sh")
		.arg("-c")
		.arg(format!(
			"diesel database reset --database-url \"{}\"",
			db_url
		))
		.stdin(Stdio::null())
		.stdout(Stdio::inherit())
		.status()
		.expect("failed to run process");
	assert!(status.success(), "failed to reset database");
}

/// Creates a rocket::local::Client for testing purposes. The rocket instance
/// will be configured with a Sqlite database located in a tmpdir  This
/// executes the given function with the Client a connection to that
/// database. The database and its directory will be removed after the function
/// has run.
pub fn with<F>(run: F)
where F: FnOnce(Client) -> () {
	let mut cfg = HashMap::new();
	cfg.insert("template_dir".into(), "src/views/".into());

	let db_url = "mysql://zauth:zauth@localhost/zauth_test";
	reset_db(db_url);

	let cfg_str = format!("mysql_database = {{ url = \"{}\" }}", db_url);
	let databases: Value = Value::from_str(&cfg_str).unwrap();
	cfg.insert("databases".into(), databases);

	let mut config = Config::development();
	config.set_extras(cfg);

	let client = Client::new(zauth::prepare_custom(config))
		.expect("valid rocket instance");

	run(client);
}

pub fn with_admin<F>(run: F)
where F: FnOnce(Client) -> () {
	with(|client| {
		{
			let db = db(&client);
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
		}

		{
			let response = client
				.post("/login")
				.body("username=admin&password=admin")
				.header(ContentType::Form)
				.dispatch();
			assert_eq!(response.status(), Status::SeeOther, "login failed");
		}

		run(client);
	});
}
