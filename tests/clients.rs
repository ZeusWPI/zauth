extern crate diesel;
extern crate rocket;

use rocket::http::ContentType;
use rocket::http::Accept;
use rocket::http::Status;

mod common;

use crate::common::url;

#[test]
fn create_client() {
	common::as_admin(|http_client, _db, _user| {
		let client_name = "test";
		let client_redirect_uri = "https://example.com/redirect";

		let client_form = format!(
			"name={}&needs_grant=true&redirect_uri_list={}",
			url(&client_name),
			url(&client_redirect_uri)
		);

		let response = http_client
			.post("/clients")
			.body(client_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch();

		assert_eq!(response.status(), Status::Created);
	});
}
