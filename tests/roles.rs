use common::{HttpClient, url};
use rocket::http::{Accept, ContentType, Status};
use zauth::models::{
	client::{Client, NewClient},
	role::{NewRole, Role},
	user::User,
};

mod common;

#[rocket::async_test]
async fn list_roles_as_user() {
	common::as_user(async move |http_client: HttpClient, _db, _user: User| {
		let response = http_client.get("/roles").dispatch().await;

		assert_eq!(response.status(), Status::Forbidden);
	})
	.await;
}

#[rocket::async_test]
async fn list_roles_as_admin() {
	common::as_admin(async move |http_client: HttpClient, _db, _user: User| {
		let response = http_client.get("/roles").dispatch().await;

		assert_eq!(response.status(), Status::Ok);
	})
	.await;
}

#[rocket::async_test]
async fn create_role_as_user() {
	common::as_user(async move |http_client: HttpClient, _db, _user: User| {
		let role_name = "test";
		let role_form =
			format!("name={role_name}&description=test_description");
		let response = http_client
			.post("/roles")
			.body(role_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Forbidden);
	})
	.await;
}

#[rocket::async_test]
async fn create_global_role() {
	common::as_admin(async move |http_client: HttpClient, db, _user| {
		let role_name = "test";
		let role_form =
			format!("name={role_name}&description=test_description");

		let response = http_client
			.post("/roles")
			.body(role_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Created);

		let json: Role = response.into_json().await.unwrap();

		let created = Role::find(json.id, &db).await.unwrap();

		assert_eq!(created.name, role_name);
		assert_eq!(created.description, "test_description");
		assert_eq!(created.client_id, None);
	})
	.await;
}

#[rocket::async_test]
async fn create_client_role() {
	common::as_admin(async move |http_client: HttpClient, db, _user| {
		let client = Client::create(
			NewClient {
				name: String::from("test"),
			},
			&db,
		)
		.await
		.unwrap();

		let role_name = "test";
		let role_form = format!(
			"name={role_name}&description=test_description&client_id={}",
			client.id
		);

		let response = http_client
			.post("/roles")
			.body(role_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Created);

		let json: Role = response.into_json().await.unwrap();

		let created = Role::find(json.id, &db).await.unwrap();

		assert_eq!(created.name, role_name);
		assert_eq!(created.description, "test_description");
		assert_eq!(created.client_id, Some(client.id));
	})
	.await;
}

#[rocket::async_test]
async fn show_role_as_user() {
	common::as_user(async move |http_client: HttpClient, db, _user: User| {
		let role = Role::create(
			NewRole {
				name: "test".into(),
				description: "test".into(),
				client_id: None,
			},
			&db,
		)
		.await
		.unwrap();

		let response = http_client
			.get(format!("/roles/{}", role.id))
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Forbidden);
	})
	.await;
}

#[rocket::async_test]
async fn show_role_as_admin() {
	common::as_admin(async move |http_client: HttpClient, db, _user: User| {
		let role = Role::create(
			NewRole {
				name: "test".into(),
				description: "test".into(),
				client_id: None,
			},
			&db,
		)
		.await
		.unwrap();

		let response = http_client
			.get(format!("/roles/{}", role.id))
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Ok);
	})
	.await;
}
#[rocket::async_test]
async fn delete_role() {
	common::as_admin(async move |http_client: HttpClient, db, _user: User| {
		let role = Role::create(
			NewRole {
				name: "test".into(),
				description: "test".into(),
				client_id: None,
			},
			&db,
		)
		.await
		.unwrap();

		let response = http_client
			.delete(format!("/roles/{}", role.id))
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::SeeOther);

		assert!(Role::find(role.id, &db).await.is_err());
	})
	.await;
}

#[rocket::async_test]
async fn add_user_to_role_as_user() {
	common::as_user(async move |http_client: HttpClient, db, user: User| {
		let role = Role::create(
			NewRole {
				name: "test".into(),
				description: "test".into(),
				client_id: None,
			},
			&db,
		)
		.await
		.unwrap();

		let role_form = format!("username={}", url(&user.username));

		let response = http_client
			.post(format!("/roles/{}/users", role.id))
			.body(role_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Forbidden);
	})
	.await;
}

#[rocket::async_test]
async fn add_user_to_role_as_admin() {
	common::as_admin(async move |http_client: HttpClient, db, user: User| {
		let role = Role::create(
			NewRole {
				name: "test".into(),
				description: "test".into(),
				client_id: None,
			},
			&db,
		)
		.await
		.unwrap();

		let role_form = format!("username={}", url(&user.username));

		let response = http_client
			.post(format!("/roles/{}/users", role.id))
			.body(role_form)
			.header(ContentType::Form)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::SeeOther);

		let users = role.clone().users(&db).await.unwrap();
		assert_eq!(users.len(), 1);
		assert_eq!(users[0].id, user.id);

		let response = http_client
			.delete(format!("/roles/{}/users/{}", role.id, user.id))
			.dispatch()
			.await;
		assert_eq!(response.status(), Status::SeeOther);

		let users = role.clone().users(&db).await.unwrap();
		assert_eq!(users.len(), 0);
	})
	.await;
}

#[rocket::async_test]
async fn add_role_to_user_as_user() {
	common::as_user(async move |http_client: HttpClient, db, user: User| {
		let role = Role::create(
			NewRole {
				name: "test".into(),
				description: "test".into(),
				client_id: None,
			},
			&db,
		)
		.await
		.unwrap();

		let role_form = format!("role_id={}", role.id);

		let response = http_client
			.post(format!("/users/{}/roles", user.username))
			.body(role_form)
			.header(ContentType::Form)
			.header(Accept::JSON)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Forbidden);
	})
	.await;
}

#[rocket::async_test]
async fn add_role_to_user_as_admin() {
	common::as_admin(async move |http_client: HttpClient, db, user: User| {
		let role = Role::create(
			NewRole {
				name: "test".into(),
				description: "test".into(),
				client_id: None,
			},
			&db,
		)
		.await
		.unwrap();

		let role_form = format!("role_id={}", role.id);

		let response = http_client
			.post(format!("/users/{}/roles", user.username))
			.body(role_form)
			.header(ContentType::Form)
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::SeeOther);

		let users = role.clone().users(&db).await.unwrap();
		assert_eq!(users.len(), 1);
		assert_eq!(users[0].id, user.id);
	})
	.await;
}
