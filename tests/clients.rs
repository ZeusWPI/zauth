#![feature(async_closure)]

extern crate diesel;
extern crate rocket;

use zauth::models::client::Client;

use rocket::http::Accept;
use rocket::http::ContentType;
use rocket::http::Status;

mod common;

use crate::common::url;

#[rocket::async_test]
async fn create_and_update_client() {
	common::as_admin(async move |http_client, db, _user| {
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

		let created = Client::find_by_name(client_name.to_owned(), &db)
			.await
			.unwrap();

		let client_form = "needs_grant=false&needs_grant=on".to_owned();

		let response = http_client
			.put(format!("/clients/{}", created.id))
			.body(client_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::NoContent);

		let updated = Client::find_by_name(client_name.to_owned(), &db)
			.await
			.unwrap();

		assert!(updated.needs_grant);

		let client_form = "name=test2".to_owned();

		let response = http_client
			.put(format!("/clients/{}", created.id))
			.body(client_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::NoContent);

		let updated =
			Client::find_by_name("test2".to_owned(), &db).await.unwrap();

		assert!(updated.needs_grant);
		assert_eq!(updated.name, "test2".to_owned());
	})
	.await;
}
