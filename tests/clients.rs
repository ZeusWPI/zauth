#![feature(async_closure)]

extern crate diesel;
extern crate rocket;

use rocket::http::Accept;
use rocket::http::ContentType;
use rocket::http::Status;

mod common;

use crate::common::url;

#[rocket::async_test]
async fn create_and_update_client() {
	common::as_admin(async move |http_client, _db, _user| {
		let client_name = "test";

		let client_form = format!("name={}", url(&client_name),);

		let response = http_client
			.post("/clients")
			.body(client_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Created);
	})
	.await;
}
