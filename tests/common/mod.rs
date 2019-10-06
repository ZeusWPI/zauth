extern crate diesel;
extern crate rocket;
extern crate tempfile;

use diesel::prelude::*;
use rocket::config::{Config, Value};
use rocket::local::Client;
use std::collections::HashMap;
use std::str::FromStr;

/// Creates a rocket::local::Client for testing purposes. The rocket instance
/// will be configured with a Sqlite database located in a tmpdir  This
/// executes the given function with the Client a connection to that
/// database. The database and its directory will be removed after the function
/// has run.
pub fn with_client_db<F>(run: F)
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
