extern crate chrono;
extern crate diesel;
extern crate rocket;

use chrono::{Duration, Utc};
use common::HttpClient;
use rocket::http::Status;
use zauth::models::session::*;

mod common;

#[rocket::async_test]
async fn valid_user_session() {
	common::as_user(async move |http_client: HttpClient, _db, _user| {
		let response = http_client.get("/current_user").dispatch().await;
		assert_eq!(response.status(), Status::Ok);
	})
	.await;
}

#[rocket::async_test]
async fn invalid_user_session() {
	common::as_user(async move |http_client: HttpClient, db, _user| {
		let mut session = Session::last(&db).await.expect("last session");
		assert!(session.valid);

		session.valid = false;
		session.update(&db).await.expect("invalidate session");

		let response = http_client.get("/current_user").dispatch().await;
		assert_eq!(response.status(), Status::Unauthorized);
	})
	.await;
}

#[rocket::async_test]
async fn expired_user_session() {
	common::as_user(async move |http_client: HttpClient, db, _user| {
		let mut session = Session::last(&db).await.expect("last session");
		assert!(session.valid);

		session.expires_at = Utc::now().naive_utc() - Duration::minutes(1);
		session
			.update(&db)
			.await
			.expect("update session expires_at");

		let response = http_client.get("/current_user").dispatch().await;
		assert_eq!(response.status(), Status::Unauthorized);
	})
	.await;
}
