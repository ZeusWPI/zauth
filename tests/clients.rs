extern crate diesel;
extern crate rocket;

use rocket::http::Accept;
use rocket::http::ContentType;
use rocket::http::Status;

mod common;

use crate::common::url;

#[test]
fn create_and_update_client() {
	common::as_admin(|http_client, _db, _user| {
		let client_name = "test";
		let client_redirect_uri = "https://example.com/redirect";

		let client_form = format!("name={}", url(&client_name),);

		let response = http_client
			.post("/clients")
			.body(client_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch();

		assert_eq!(response.status(), Status::Created);
	});
}
