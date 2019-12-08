extern crate diesel;
extern crate rocket;

use rocket::http::Status;

mod common;

#[test]
fn should_get_all_users() {
	common::with(|http_client| {
		let response = http_client.get("/users").dispatch();

		assert_eq!(response.status(), Status::Ok);
	});
}
