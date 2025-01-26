#![feature(async_closure)]

extern crate diesel;
extern crate rocket;

use common::HttpClient;
use rocket::http::ContentType;
use rocket::http::Status;
use zauth::models::user::User;

mod common;

#[rocket::async_test]
async fn register_passkey_as_visitor() {
	common::as_visitor(async move |http_client: HttpClient, _db| {
		let response = http_client
			.post("/webauthn/start_register")
			.header(ContentType::JSON)
			.body("true")
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Unauthorized);
	})
	.await;
}

#[rocket::async_test]
async fn list_passkeys_as_visitor() {
	common::as_visitor(async move |http_client: HttpClient, _db| {
		let response = http_client.get("/passkeys").dispatch().await;

		assert_eq!(response.status(), Status::Unauthorized);
	})
	.await;
}

#[rocket::async_test]
async fn list_passkeys_as_user() {
	common::as_user(async move |http_client: HttpClient, _db, _user: User| {
		let response = http_client.get("/passkeys").dispatch().await;

		assert_eq!(response.status(), Status::Ok);
	})
	.await;
}
