extern crate diesel;
extern crate rocket;

use diesel::prelude::*;
use rocket::local::Client;

pub fn create_http_client() -> Client {
	Client::new(zauth::rocket()).expect("valid rocket instance")
}

pub fn db() -> SqliteConnection {
	SqliteConnection::establish("sqlite:db/db.sqlite").unwrap()
}
