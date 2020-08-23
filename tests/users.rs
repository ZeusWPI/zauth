extern crate diesel;
extern crate rocket;

use rocket::http::{Accept, ContentType, Status};

use pwhash::bcrypt;
use zauth::models::user::*;

mod common;

#[test]
fn get_all_users() {
	common::as_visitor(|http_client, _db| {
		let response = http_client.get("/users").dispatch();
		assert_eq!(response.status(), Status::Unauthorized);
	});

	common::as_user(|http_client, _db, _user| {
		let response = http_client.get("/users").dispatch();
		assert_eq!(response.status(), Status::Ok);
	});

	common::as_admin(|http_client, _db, _admin| {
		let response = http_client.get("/users").dispatch();

		assert_eq!(response.status(), Status::Ok);
	});
}

#[test]
fn show_user_as_visitor() {
	common::as_visitor(|http_client, _db| {
		let response = http_client.get("/users/1").dispatch();
		assert_eq!(
			response.status(),
			Status::Unauthorized,
			"visitor should get unauthrorized"
		);
	});
}

#[test]
fn show_user_as_user() {
	common::as_user(|http_client, db, user| {
		let other = User::create(
			NewUser {
				username:   String::from("somebody"),
				password:   String::from("once"),
				first_name: String::from("told"),
				last_name:  String::from("me"),
				email:      String::from("zeus"),
				ssh_key:    Some(String::from("would be forever")),
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		let response =
			http_client.get(format!("/users/{}", other.id)).dispatch();

		assert_eq!(
			response.status(),
			Status::NotFound,
			"should not be able to see other user's profile"
		);

		let response =
			http_client.get(format!("/users/{}", user.id)).dispatch();

		assert_eq!(
			response.status(),
			Status::Ok,
			"should be able to see own profile"
		);
	});
}

#[test]
fn show_user_as_admin() {
	common::as_admin(|http_client, db, admin| {
		let other = User::create(
			NewUser {
				username:   String::from("somebody"),
				password:   String::from("once"),
				first_name: String::from("told"),
				last_name:  String::from("me"),
				email:      String::from("zeus"),
				ssh_key:    Some(String::from("would be forever")),
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		let response =
			http_client.get(format!("/users/{}", other.id)).dispatch();

		assert_eq!(
			response.status(),
			Status::Ok,
			"admin should see other's profile"
		);

		let response =
			http_client.get(format!("/users/{}", admin.id)).dispatch();

		assert_eq!(
			response.status(),
			Status::Ok,
			"admin should see own profile"
		);
	});
}

#[test]
fn update_self() {
	common::as_user(|http_client, db, user| {
		let response = http_client
			.put(format!("/users/{}", user.id))
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("username=newusername")
			.dispatch();

		assert_eq!(
			response.status(),
			Status::NoContent,
			"user should be able to edit themself"
		);

		let updated = User::find(user.id, &db).unwrap();

		assert_eq!("newusername", updated.username);

		let other = User::create(
			NewUser {
				username:   String::from("somebody"),
				password:   String::from("once"),
				first_name: String::from("told"),
				last_name:  String::from("me"),
				email:      String::from("zeus"),
				ssh_key:    Some(String::from("would be forever")),
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		let response = http_client
			.put(format!("/users/{}", other.id))
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("username=newusername")
			.dispatch();

		assert_eq!(
			response.status(),
			Status::Forbidden,
			"user should not be able to edit others"
		);
	});
}

#[test]
fn change_password() {
	common::as_user(|http_client, db, user| {
		let response = http_client
			.put(format!("/users/{}", user.id))
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("password=newpassword")
			.dispatch();

		assert_eq!(
			response.status(),
			Status::NoContent,
			"user should be able to change password"
		);

		let updated = User::find(user.id, &db).unwrap();

		assert_ne!(
			user.hashed_password, updated.hashed_password,
			"password should have changed"
		);
	});
}

#[test]
fn make_admin() {
	common::as_admin(|http_client, db, _admin| {
		let other = User::create(
			NewUser {
				username:   String::from("somebody"),
				password:   String::from("once"),
				first_name: String::from("told"),
				last_name:  String::from("me"),
				email:      String::from("zeus"),
				ssh_key:    Some(String::from("would be forever")),
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		let response = http_client
			.post(format!("/users/{}/admin", other.id))
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("admin=true")
			.dispatch();

		assert_eq!(
			response.status(),
			Status::NoContent,
			"admin should be able to make other admin"
		);

		let updated = User::find(other.id, &db).unwrap();

		assert!(updated.admin, "other user should be admin now");
	});
}

#[test]
fn try_make_admin() {
	common::as_user(|http_client, db, _user| {
		let other = User::create(
			NewUser {
				username:   String::from("somebody"),
				password:   String::from("once"),
				first_name: String::from("told"),
				last_name:  String::from("me"),
				email:      String::from("zeus"),
				ssh_key:    Some(String::from("would be forever")),
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		let response = http_client
			.post(format!("/users/{}/admin", other.id))
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body("admin=true")
			.dispatch();

		assert_eq!(
			response.status(),
			Status::Forbidden,
			"user should not be able to make other admin"
		);
	});
}

#[test]
fn create_user_form() {
	common::as_admin(|http_client, db, _admin| {
		let user_count = User::all(&db).unwrap().len();

		let response = http_client
			.post("/users")
			.header(ContentType::Form)
			.header(Accept::JSON)
			.body(
				"username=testuser&password=testpassword&first_name=abc&\
				 last_name=def&email=hij@klm.op&ssh_key=qrs",
			)
			.dispatch();

		assert_eq!(response.status(), Status::Ok);

		assert_eq!(user_count + 1, User::all(&db).unwrap().len());

		let last_created = User::last(&db).unwrap();
		assert_eq!("testuser", last_created.username);
	});
}

#[test]
fn create_user_json() {
	common::as_admin(|http_client, db, _admin| {
		let user_count = User::all(&db).unwrap().len();

		let response = http_client
			.post("/users")
			.header(ContentType::JSON)
			.header(Accept::JSON)
			.body(
				"{\"username\": \"testuser\", \"password\": \"testpassword\", \
				 \"first_name\": \"abc\", \"last_name\": \"def\", \"email\": \
				 \"hij@klm.op\", \"ssh_key\": \"qrs\"}",
			)
			.dispatch();

		assert_eq!(response.status(), Status::Ok);

		assert_eq!(user_count + 1, User::all(&db).unwrap().len());

		let last_created = User::last(&db).unwrap();
		assert_eq!("testuser", last_created.username);
	});
}

#[test]
fn forgot_password() {
	common::as_visitor(|http_client, db| {
		let email = String::from("test@example.com");
		let user = User::create(
			NewUser {
				username:   String::from("a"),
				password:   String::from("b"),
				first_name: String::from("c"),
				last_name:  String::from("d"),
				email:      email.clone(),
				ssh_key:    None,
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		assert!(user.password_reset_token.is_none());
		assert!(user.password_reset_expiry.is_none());

		let response = http_client
			.get("/users/forgot_password")
			.header(Accept::HTML)
			.dispatch();

		assert_eq!(
			response.status(),
			Status::Ok,
			"should get forgot password page"
		);

		let response = common::expect_mail_to(vec![&email], || {
			http_client
				.post("/users/forgot_password")
				.header(ContentType::Form)
				.header(Accept::HTML)
				.body(format!("for_email={}", &email))
				.dispatch()
		});

		assert_eq!(
			response.status(),
			Status::Ok,
			"should post email to forgot password"
		);

		let user = user.reload(&db).unwrap();

		assert!(user.password_reset_token.is_some());
		assert!(user.password_reset_expiry.is_some());

		let token = user.password_reset_token.clone().unwrap();

		let response = http_client
			.get(format!("/users/reset_password/{}", token,))
			.header(Accept::HTML)
			.dispatch();

		assert_eq!(
			response.status(),
			Status::Ok,
			"should get reset password page"
		);

		let old_password_hash = user.hashed_password.clone();
		let new_password = "passw0rd";

		dbg!(&user);

		let response = common::expect_mail_to(vec![&email], || {
			http_client
				.post(format!("/users/reset_password/"))
				.header(ContentType::Form)
				.header(Accept::HTML)
				.body(format!(
					"token={}&new_password={}",
					&token, &new_password
				))
				.dispatch()
		});

		dbg!(&user);

		assert_eq!(
			response.status(),
			Status::Ok,
			"should post to reset password page"
		);

		let user = user.reload(&db).unwrap();

		assert!(user.password_reset_token.is_none());
		assert!(user.password_reset_expiry.is_none());
		assert_ne!(user.hashed_password, old_password_hash);
		assert!(bcrypt::verify(new_password, &user.hashed_password));
	});
}

#[test]
fn forgot_password_non_existing_email() {
	common::as_visitor(|http_client, db| {
		let email = String::from("test@example.com");
		let _user = User::create(
			NewUser {
				username:   String::from("a"),
				password:   String::from("b"),
				first_name: String::from("c"),
				last_name:  String::from("d"),
				email:      email.clone(),
				ssh_key:    None,
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		let response = common::dont_expect_mail(|| {
			http_client
				.post("/users/forgot_password")
				.header(ContentType::Form)
				.header(Accept::HTML)
				.body("for_email=not_this_email@example.com")
				.dispatch()
		});

		assert_eq!(
			response.status(),
			Status::Ok,
			"should still say everything is OK, even when email does not exist"
		);
	});
}

#[test]
fn reset_password_invalid_token() {
	common::as_visitor(|http_client, db| {
		let email = String::from("test@example.com");
		let user = User::create(
			NewUser {
				username:   String::from("a"),
				password:   String::from("b"),
				first_name: String::from("c"),
				last_name:  String::from("d"),
				email:      email.clone(),
				ssh_key:    None,
			},
			common::BCRYPT_COST,
			&db,
		)
		.unwrap();

		let response = http_client
			.post("/users/forgot_password")
			.header(ContentType::Form)
			.header(Accept::HTML)
			.body(format!("for_email={}", &email))
			.dispatch();

		assert_eq!(response.status(), Status::Ok);

		let user = user.reload(&db).unwrap();
		let token = user.password_reset_token.clone().unwrap();
		let old_hash = user.hashed_password.clone();

		let response = common::dont_expect_mail(|| {
			http_client
				.post("/users/reset_password/")
				.header(ContentType::Form)
				.header(Accept::HTML)
				.body(format!(
					"token=not{}&new_password={}",
					&token, "passw0rd"
				))
				.dispatch()
		});

		assert_eq!(response.status(), Status::Forbidden);

		let user = user.reload(&db).unwrap();
		assert_eq!(user.hashed_password, old_hash);
	});
}
