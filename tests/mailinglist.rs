#![feature(async_closure)]

extern crate diesel;
extern crate rocket;

use lettre::message::Mailbox;
use rocket::http::{Accept, ContentType, Status};

use zauth::config::Config;
use zauth::mailer::Mailer;
use zauth::models::mail::NewMail;
use zauth::models::user::*;
use zauth::DbConn;

mod common;

const TEST_USERS: [(&'static str, UserState, bool); 10] = [
	("valid0", UserState::Active, true),
	("valid1", UserState::Active, true),
	("pending_approval0", UserState::PendingApproval, true),
	("pending_approval1", UserState::PendingApproval, true),
	("pending_email0", UserState::PendingMailConfirmation, true),
	("pending_email1", UserState::PendingMailConfirmation, true),
	("disabled0", UserState::Disabled, true),
	("disabled1", UserState::Disabled, true),
	("unsubbed0", UserState::Disabled, false),
	("unsubbed01", UserState::Disabled, false),
];

async fn setup_test_users(db: &DbConn) {
	for test_user in TEST_USERS {
		let mut created_user = User::create(
			NewUser {
				username:    test_user.0.to_string(),
				password:    format!(
					"{}verylongandsecurepassword",
					test_user.0.to_string()
				),
				full_name:   test_user.0.to_string(),
				email:       format!("{}@example.com", test_user.0.to_string()),
				ssh_key:     None,
				not_a_robot: true,
			},
			common::BCRYPT_COST,
			&db,
		)
		.await
		.unwrap();

		created_user.state = test_user.1;
		created_user.subscribed_to_mailing_list = test_user.2;

		created_user.update(&db).await.unwrap();
	}
}

/// Check if the correct users are selected as being validly subscribed to the
/// mailing list and if mailing list emails have them in bcc properly
#[rocket::async_test]
async fn get_valid_subscribed_users() {
	common::as_visitor(async move |_client, db| {
		setup_test_users(&db).await;

		let subscribed_users = User::find_subscribed(&db).await.unwrap();

		let subscribed_usernames: Vec<String> = subscribed_users
			.iter()
			.map(|u| u.username.clone())
			.collect();

		let bcc: Vec<Mailbox> = subscribed_users
			.iter()
			.map(|u| Mailbox::try_from(u).unwrap())
			.collect();

		let test_cfg = Config {
			admin_email: "".to_string(),
			user_session_seconds: 0,
			client_session_seconds: 0,
			authorization_token_seconds: 0,
			email_confirmation_token_seconds: 0,
			secure_token_length: 0,
			bcrypt_cost: 0,
			base_url: "".to_string(),
			mail_queue_size: 5,
			mail_queue_wait_seconds: 5,
			mail_from: "test@example.com".to_string(),
			mail_server: "stub".to_string(),
			mailing_list_name: "Leden".to_string(),
			mailing_list_email: "leden@zeus.ugent.be".to_string(),
			maximum_pending_users: 0,
			webhook_url: None,
		};
		let test_mailer = Mailer::new(&test_cfg).unwrap();

		let test_receiver = Mailbox::new(
			Some(test_cfg.mailing_list_name),
			test_cfg.mailing_list_email.parse().unwrap(),
		);

		let test_mail = test_mailer
			.build_with_bcc(
				test_receiver,
				bcc,
				"foosubject".to_string(),
				"foobody".to_string(),
			)
			.unwrap();

		let bcc_header = test_mail.headers().get_raw("Bcc");

		assert_eq!(
			bcc_header,
			Some("valid0 <valid0@example.com>, valid1 <valid1@example.com>"),
			"incorrect bcc header",
		);

		assert_eq!(
			subscribed_usernames,
			vec!["valid0".to_string(), "valid1".to_string()],
			"did not get correct subscribed users"
		);
	})
	.await;
}

/// Ensure that only logged-in users can unsubscribe
#[rocket::async_test]
async fn visitor_cannot_unsubscribe() {
	common::as_visitor(async move |http_client, _db| {
		let response = http_client.get("/users/unsubscribe").dispatch().await;
		assert_eq!(
			response.status(),
			Status::Unauthorized,
			"visitors should not be able to see unsubscribe page"
		);

		let response = http_client
			.post("/users/unsubscribe")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("token=foo")
			.dispatch()
			.await;
		assert_eq!(
			response.status(),
			Status::Unauthorized,
			"visitors should not be able unsubscribe"
		)
	})
	.await;
}

/// Ensure that users can use unsubscribe endpoints
#[rocket::async_test]
async fn user_can_unsubscribe() {
	common::as_user(async move |http_client, _db, _user| {
		let response = http_client.get("/users/unsubscribe").dispatch().await;
		assert_eq!(
			response.status(),
			Status::Ok,
			"users should be able to see unsubscribe page"
		);

		let response = http_client
			.post("/users/unsubscribe")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("token=foo")
			.dispatch()
			.await;
		assert_eq!(
			response.status(),
			Status::Ok,
			"users should be able to unsubscribe"
		)
	})
	.await;
}

/// Ensure that admins can use unsubscribe endpoints
#[rocket::async_test]
async fn admin_can_unsubscribe() {
	common::as_admin(async move |http_client, _db, _user| {
		let response = http_client.get("/users/unsubscribe").dispatch().await;
		assert_eq!(
			response.status(),
			Status::Ok,
			"admins should be able to see unsubscribe page"
		);

		let response = http_client
			.post("/users/unsubscribe")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("token=foo")
			.dispatch()
			.await;
		assert_eq!(
			response.status(),
			Status::Ok,
			"admins should be able to unsubscribe"
		)
	})
	.await;
}

/// Ensure visitors cannot see mails pages
#[rocket::async_test]
async fn visitor_cannot_use_mailinglist() {
	common::as_visitor(async move |http_client, _db| {
		let mails_response = http_client.get("/mails").dispatch().await;
		let new_mail_response = http_client.get("/mails/new").dispatch().await;
		let specific_mail_response =
			http_client.get("/mails/0").dispatch().await;
		let create_mail_response = http_client
			.post("/mails")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("subject=foosubject&body=foobody")
			.dispatch()
			.await;

		assert_eq!(
			mails_response.status(),
			Status::Unauthorized,
			"visitors should not be able to see mails overview page"
		);
		assert_eq!(
			new_mail_response.status(),
			Status::Unauthorized,
			"visitors should not be able to see new mail page"
		);
		assert_eq!(
			specific_mail_response.status(),
			Status::Unauthorized,
			"visitors should not be able to see specific mail page"
		);
		assert_eq!(
			create_mail_response.status(),
			Status::Unauthorized,
			"visitors should not be able to create mails"
		);
	})
	.await;
}

/// Ensure users cannot see mails pages
#[rocket::async_test]
async fn user_cannot_use_mailinglist() {
	common::as_user(async move |http_client, _db, _user| {
		let mails_response = http_client.get("/mails").dispatch().await;
		let new_mail_response = http_client.get("/mails/new").dispatch().await;
		let specific_mail_response =
			http_client.get("/mails/0").dispatch().await;
		let create_mail_response = http_client
			.post("/mails")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("subject=foosubject&body=foobody")
			.dispatch()
			.await;

		assert_eq!(
			mails_response.status(),
			Status::Forbidden,
			"users should not be able to see mails overview page"
		);
		assert_eq!(
			new_mail_response.status(),
			Status::Forbidden,
			"users should not be able to see new mail page"
		);
		assert_eq!(
			specific_mail_response.status(),
			Status::Forbidden,
			"users should not be able to see specific mail page"
		);
		assert_eq!(
			create_mail_response.status(),
			Status::Forbidden,
			"users should not be able to create mails"
		);
	})
	.await;
}

/// Ensure admins can see mails pages
#[rocket::async_test]
async fn admin_can_use_mailinglist() {
	common::as_admin(async move |http_client, db, _user| {
		let test_mail = NewMail {
			subject: "foo".to_string(),
			body:    "bar".to_string(),
		};
		let test_mail = test_mail.save(&db).await.unwrap();

		let mails_response = http_client.get("/mails").dispatch().await;
		let new_mail_response = http_client.get("/mails/new").dispatch().await;
		let specific_mail_response = http_client
			.get(format!("/mails/{}", test_mail.id))
			.dispatch()
			.await;
		let create_mail_response = http_client
			.post("/mails")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("subject=foosubject&body=foobody")
			.dispatch()
			.await;

		assert_eq!(
			mails_response.status(),
			Status::Ok,
			"admins should be able to see mails overview page"
		);
		assert_eq!(
			new_mail_response.status(),
			Status::Ok,
			"admins should be able to see new mail page"
		);
		assert_eq!(
			specific_mail_response.status(),
			Status::Ok,
			"admins should be able to see specific mail page"
		);
		assert_eq!(
			create_mail_response.status(),
			Status::Ok,
			"admins should be able to create mails"
		);
	})
	.await;
}
