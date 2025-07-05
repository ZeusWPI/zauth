extern crate diesel;
extern crate rocket;

use common::HttpClient;
use rocket::http::{Accept, ContentType, Status};

use zauth::DbConn;
use zauth::models::mail::NewMail;
use zauth::models::user::*;

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
	("unsubbed0", UserState::Active, false),
	("unsubbed01", UserState::Active, false),
];

async fn setup_test_users(db: &DbConn) {
	for test_user in TEST_USERS {
		let mut created_user = User::create(
			NewUser {
				username: test_user.0.to_string(),
				password: format!(
					"{}verylongandsecurepassword",
					test_user.0.to_string()
				),
				full_name: test_user.0.to_string(),
				email: format!("{}@example.com", test_user.0.to_string()),
				ssh_key: None,
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
/// mailing list and if mailing list emails are sent to the correct users
///
/// Also asserts that admin accounts can use the POST /mails endpoint. Normally
/// this would've gone in `admin_can_use_mailinglist`, but doing so can cause
/// test to randomly fail due to the `common::as_admin` wrapper inserting a
/// new, subscribed user
#[rocket::async_test]
async fn mailinglist_workflow() {
	common::as_admin(async move |http_client: HttpClient, db, admin: User| {
		setup_test_users(&db).await;

		let subscribed_users = User::find_subscribed(&db).await.unwrap();

		let receivers =
			subscribed_users.iter().map(|u| u.email.as_str()).collect();

		let subscribed_usernames: Vec<String> = subscribed_users
			.iter()
			.map(|u| u.username.clone())
			.collect();

		assert_eq!(
			subscribed_usernames,
			vec![
				"admin".to_string(),
				"valid0".to_string(),
				"valid1".to_string()
			],
			"did not get correct subscribed users"
		);

		let create_mails_response =
			common::expect_mails_to(receivers, async || {
				http_client
					.post("/mails")
					.header(ContentType::Form)
					.header(Accept::JSON)
					.body(format!(
						"author={}&subject=foosubject&body=foobody",
						admin.username
					))
					.dispatch()
					.await
			})
			.await;

		assert_eq!(
			create_mails_response.status(),
			Status::Ok,
			"admins should be able to create mails"
		);
	})
	.await;
}

/// Ensure that anyone can unsubscribe
#[rocket::async_test]
async fn visitor_can_unsubscribe() {
	common::as_visitor(async move |http_client: HttpClient, db| {
		setup_test_users(&db).await;
		let test_user = &User::find_subscribed(&db).await.unwrap()[0];
		let test_token = &test_user.unsubscribe_token;

		let page_response = http_client
			.get(format!("/users/unsubscribe/{}", test_token))
			.dispatch()
			.await;
		assert_eq!(
			page_response.status(),
			Status::Ok,
			"should be able to see unsubscribe page"
		);

		let invalid_token_response = http_client
			.post("/users/unsubscribe")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("token=foo")
			.dispatch()
			.await;
		assert_eq!(
			invalid_token_response.status(),
			Status::Unauthorized,
			"should not be able to unsubscribe with an invalid token"
		);

		let valid_token_response = http_client
			.post("/users/unsubscribe")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body(format!("token={}", test_token))
			.dispatch()
			.await;
		assert_eq!(
			valid_token_response.status(),
			Status::Ok,
			"should be able to unsubscribe with a valid token"
		);
	})
	.await;
}

/// Ensure visitors cannot see mails pages
#[rocket::async_test]
async fn visitor_cannot_use_mailinglist() {
	common::as_visitor(async move |http_client: HttpClient, _db| {
		let mails_response = http_client.get("/mails").dispatch().await;
		let new_mail_response = http_client.get("/mails/new").dispatch().await;
		let specific_mail_response =
			http_client.get("/mails/0").dispatch().await;
		let create_mail_response = http_client
			.post("/mails")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("author=fooauthor&subject=foosubject&body=foobody")
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

/// Ensure users can see the mailinglist, but cannot create any mails
#[rocket::async_test]
async fn user_can_see_mailinglist() {
	common::as_user(async move |http_client: HttpClient, db, user: User| {
		let test_mail = NewMail {
			author: user.username,
			subject: "foo".to_string(),
			body: "bar".to_string(),
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
			.body("author=fooauthor&subject=foosubject&body=foobody")
			.dispatch()
			.await;

		assert_eq!(
			mails_response.status(),
			Status::Ok,
			"users should be able to see mails overview page"
		);
		assert_eq!(
			new_mail_response.status(),
			Status::Forbidden,
			"users should not be able to see new mail page"
		);
		assert_eq!(
			specific_mail_response.status(),
			Status::Ok,
			"users should be able to see specific mail page"
		);
		assert_eq!(
			create_mail_response.status(),
			Status::Forbidden,
			"users should not be able to create mails"
		);
	})
	.await;
}

/// Ensure admins can see mails pages and create new mails
#[rocket::async_test]
async fn admin_can_use_mailinglist() {
	common::as_admin(async move |http_client: HttpClient, db, user: User| {
		let test_mail = NewMail {
			author: user.username,
			subject: "foo".to_string(),
			body: "bar".to_string(),
		};
		let test_mail = test_mail.save(&db).await.unwrap();

		let mails_response = http_client.get("/mails").dispatch().await;
		let new_mail_response = http_client.get("/mails/new").dispatch().await;
		let specific_mail_response = http_client
			.get(format!("/mails/{}", test_mail.id))
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
	})
	.await;
}
