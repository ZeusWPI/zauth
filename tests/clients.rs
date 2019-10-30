extern crate diesel;
extern crate rocket;

use rocket::http::ContentType;
use rocket::http::Status;

mod common;

use common::url;

#[test]
fn should_create_client() {
	common::with_admin(|http_client| {
		let client_name = "test";
		let client_redirect_uri = "https://example.com/redirect";

		let client_form = format!(
			"name={}&needs_grant=true&redirect_uri_list={}",
			url(&client_name),
			url(&client_redirect_uri)
		);

		let mut response = http_client
			.post("/clients")
			.body(client_form)
			.header(ContentType::Form)
			.dispatch();

		assert_eq!(response.status(), Status::Created);
	});
}
