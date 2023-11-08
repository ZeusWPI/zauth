#![feature(async_closure)]

extern crate diesel;
extern crate rocket;

use rocket::http::Accept;
use rocket::http::ContentType;
use rocket::http::Status;

mod common;

use crate::common::{config, url};
use zauth::models::client::{Client, NewClient};
use zauth::models::session::Session;

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

#[rocket::async_test]
async fn change_client_secret() {
	common::as_admin(async move |http_client, db, _user| {
		let client = Client::create(
			NewClient {
				name: "test".to_string(),
			},
			&db,
		)
		.await
		.expect("create client");

		let secret_pre = client.secret.clone();
		assert!(secret_pre.len() > 5);

		let response = http_client
			.post(format!("/clients/{}/generate_secret", &client.id))
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::NoContent);

		let client = client.reload(&db).await.expect("reload client");
		assert_ne!(secret_pre, client.secret);
	})
	.await;
}

#[rocket::async_test]
async fn delete_client_with_session() {
	common::as_admin(async move |http_client, db, user| {
		let client_name = "test";

		let client_form = format!("name={}", url(&client_name),);

		let create = http_client
			.post("/clients")
			.body(client_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(create.status(), Status::Created);
		let client = Client::find_by_name(client_name.to_owned(), &db)
			.await
			.unwrap();

		let session =
			Session::create_client_session(&user, &client,None, &config(), &db)
				.await
				.unwrap();

		let delete = http_client
			.delete(format!("/clients/{}", &client.id))
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(delete.status(), Status::NoContent);
		assert!(Client::find(client.id, &db).await.is_err());
		assert!(Session::find_by_id(session.id, &db).await.is_err());
	})
	.await;
}
