extern crate diesel;
extern crate rocket;

use rocket::http::ContentType;
use rocket::http::Status;

use zauth::models::user::*;

mod common;

#[test]
fn get_all_users() {
	common::as_visitor(|http_client, _db| {
		let response = http_client.get("/users").dispatch();

		assert_eq!(response.status(), Status::Ok);
	});
}

#[test]
fn create_user_form() {
	common::as_admin(|http_client, db| {
		let user_count = User::all(&db).len();

		let response = http_client
			.post("/users")
			.header(ContentType::Form)
			.body("username=testuser&password=testpassword")
			.dispatch();

		assert_eq!(response.status(), Status::Ok);

		assert_eq!(user_count + 1, User::all(&db).len());

		let last_created = User::last(&db).unwrap();
		assert_eq!("testuser", last_created.username);
	});
}
