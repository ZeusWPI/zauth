extern crate diesel;
extern crate rocket;

use rocket::http::{Accept, ContentType, Status};

use zauth::models::user::*;

mod common;

#[test]
fn get_all_users() {
	common::as_visitor(|http_client, _db| {
		let response = http_client.get("/users").dispatch();
		assert_eq!(response.status(), Status::Unauthorized);
	});

	common::as_user(|http_client, _db, _user| {
		let mut response = http_client.get("/users").dispatch();
		dbg!(response.body_string());
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
				username: String::from("somebody"),
				password: String::from("once"),
				firstname: String::from("told"),
				lastname: String::from("me"),
				email: String::from("zeus"),
				ssh_key: Some(String::from("would be forever"))
			},
			&db,
		)
		.unwrap();

		let mut response =
			http_client.get(format!("/users/{}", other.id)).dispatch();
		dbg!(response.body_string());

		assert_eq!(
			response.status(),
			Status::NotFound,
			"should not be able to see other user's profile"
		);

		let mut response =
			http_client.get(format!("/users/{}", user.id)).dispatch();
		dbg!(response.body_string());

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
				username: String::from("somebody"),
				password: String::from("once"),
				firstname: String::from("told"),
				lastname: String::from("me"),
				email: String::from("zeus"),
				ssh_key: Some(String::from("would be forever"))
			},
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
				username: String::from("somebody"),
				password: String::from("once"),
				firstname: String::from("told"),
				lastname: String::from("me"),
				email: String::from("zeus"),
				ssh_key: Some(String::from("would be forever"))
			},
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

		assert!(
			user.hashed_password != updated.hashed_password,
			"password should have changed"
		);
	});
}

#[test]
fn make_admin() {
	common::as_admin(|http_client, db, _admin| {
		let other = User::create(
			NewUser {
				username: String::from("somebody"),
				password: String::from("once"),
				firstname: String::from("told"),
				lastname: String::from("me"),
				email: String::from("zeus"),
				ssh_key: Some(String::from("would be forever"))
			},
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
				username: String::from("somebody"),
				password: String::from("once"),
				firstname: String::from("told"),
				lastname: String::from("me"),
				email: String::from("zeus"),
				ssh_key: Some(String::from("would be forever"))
			},
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
			.body("username=testuser&password=testpassword&firstname=abc&lastname=def&email=hij@klm.op&ssh_key=qrs")
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
				"{\"username\": \"testuser\", \"password\": \"testpassword\", \"firstname\": \"abc\", \"lastname\": \"def\", \"email\": \"hij@klm.op\", \"ssh_key\": \"qrs\"}",
			)
			.dispatch();

		assert_eq!(response.status(), Status::Ok);

		assert_eq!(user_count + 1, User::all(&db).unwrap().len());

		let last_created = User::last(&db).unwrap();
		assert_eq!("testuser", last_created.username);
	});
}
