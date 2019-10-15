extern crate diesel;
extern crate rocket;

use rocket::http::ContentType;
use rocket::http::Header;
use rocket::http::Status;

mod common;

use common::url;

#[test]
fn should_get_all_users() {
	common::with(|http_client| {
		let mut response = http_client.get("/users").dispatch();

		assert_eq!(response.status(), Status::Ok);
	});
}
