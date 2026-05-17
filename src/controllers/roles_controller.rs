use std::vec::Vec;

use diesel::result::DatabaseErrorKind;
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::content::RawHtml;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder, status};
use rocket::serde::json::Json;

use crate::DbConn;
use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::AdminSession;
use crate::errors::{Either, InternalError, Result, ZauthError};
use crate::models::client::Client;
use crate::models::role::{NewRole, Role, RoleVisibility};
use crate::models::user::User;
use crate::views::accepter::Accepter;

#[get("/roles?<error>")]
pub async fn list_roles<'r>(
	// from url
	error: Option<String>,
	// from headers
	session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let roles: Vec<Role> = Role::all(&db).await?;

	Ok(Accepter {
		html: RawHtml(template!("roles/index.html", {
			current_user: User = session.admin,
			error: Option<String> = error,
			roles: Vec<Role> = roles.clone(),
		})),
		json: Json(roles),
	})
}

#[post("/roles", data = "<role>")]
pub async fn create_role<'r, 'a>(
	// from body
	role: Api<NewRole>,
	// from headers
	_admin: AdminSession,
	// injected
	db: DbConn,
) -> Result<
	Either<impl Responder<'a, 'static>, impl Responder<'r, 'static> + use<'r>>,
> {
	let new_role: NewRole = role.into_inner();
	let new_role_name = new_role.name.clone();
	let role = Role::create(new_role, &db).await;
	match role {
		Ok(role) => Ok(Either::Left(Accepter {
			html: Redirect::to(uri!(list_roles(None::<String>))),
			json: status::Created::new(String::from("/role")).body(Json(role)),
		})),
		Err(ZauthError::Internal(InternalError::DatabaseError(
			diesel::result::Error::DatabaseError(
				DatabaseErrorKind::UniqueViolation,
				_,
			),
		))) => Ok(Either::Right({
			let error_msg =
				format!("Role with name “{}” already exists", new_role_name);
			Accepter {
				html: Redirect::to(uri!(list_roles(Some(error_msg.clone())))),
				json: error_msg,
			}
		})),
		Err(err) => Err(err),
	}
}

#[get("/roles/<id>?<error>&<success>")]
pub async fn show_role_page<'r>(
	// from url
	id: i32,
	error: Option<String>,
	success: Option<String>,
	// from headers
	session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(id, &db).await?;

	let limited_to_clients: Vec<Client> =
		role.clone().limited_to_clients(&db).await?;
	let limited_to_client_ids: Vec<i32> =
		limited_to_clients.iter().map(|client| client.id).collect();

	let users: Vec<User> = role.clone().users(&db).await?;
	let user_ids: Vec<i32> = users.iter().map(|user| user.id).collect();

	let clients: Vec<Client> = role.clone().clients(&db).await?;
	let client_ids: Vec<i32> = clients.iter().map(|client| client.id).collect();

	let all_users: Vec<User> = User::all(&db).await?;
	let all_clients: Vec<Client> = Client::all(&db).await?;

	Ok(RawHtml(template!("roles/show_role.html", {
		current_user: User = session.admin,

		error: Option<String> = error,
		success: Option<String> = success,

		role: Role = role,
		limited_to_clients: Vec<Client> = limited_to_clients,
		limited_to_client_ids: Vec<i32> = limited_to_client_ids,
		users: Vec<User> = users,
		user_ids: Vec<i32> = user_ids,
		clients: Vec<Client> = clients,
		client_ids: Vec<i32> = client_ids,

		all_clients: Vec<Client> = all_clients,
		all_users: Vec<User> = all_users,
	})))
}

#[post("/roles/<role_id>/description", data = "<description>")]
pub async fn update_description<'r>(
	// from url
	role_id: i32,
	// from body
	description: Form<String>,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mut role: Role = Role::find(role_id, &db).await?;
	role.description = description.clone();
	role.update(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_role_page(
			role_id,
			None::<String>,
			Some("Successfully changed description")
		))),
		json: Custom(Status::Ok, ()),
	})
}

#[post("/roles/<role_id>/visibility", data = "<visibility>")]
pub async fn update_visibility<'r>(
	// from url
	role_id: i32,
	// from body
	visibility: Form<RoleVisibility>,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mut role: Role = Role::find(role_id, &db).await?;
	role.visibility = visibility.into_inner();
	let role: Role = role.update(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_role_page(
			role_id,
			None::<String>,
			Some(format!(
				"Successfully changed visibility to “{}”",
				role.visibility,
			)),
		))),
		json: Custom(Status::Ok, ()),
	})
}

#[post("/roles/<role_id>/limited_to_clients", data = "<client_id>")]
pub async fn add_limited_to_client<'r>(
	// from url
	role_id: i32,
	// from body
	client_id: Form<i32>,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let client_result = Client::find(client_id.clone(), &db).await;
	Ok(match client_result {
		Ok(client) => {
			role.add_client_to_limited_to(client.id, &db).await?;
			Accepter {
				html: Redirect::to(uri!(show_role_page(
					role.id,
					None::<String>,
					Some(format!(
						"Successfully added client “{}” to the “limited to” list",
						client.name,
					)),
				))),
				json: Custom(Status::Ok, ()),
			}
		},
		Err(ZauthError::NotFound(_)) => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("Client not found"),
				None::<String>,
			))),
			json: Custom(Status::NotFound, ()),
		},
		_ => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("An internal server error occured"),
				None::<String>,
			))),
			json: Custom(Status::InternalServerError, ()),
		},
	})
}

#[post("/roles/<role_id>/users", data = "<user_id>")]
pub async fn add_user<'r>(
	// from url
	role_id: i32,
	// from body
	user_id: Form<i32>,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let user_result = User::find(user_id.into_inner(), &db).await;
	Ok(match user_result {
		Ok(user) => {
			role.add_user(user.id, &db).await?;
			Accepter {
				html: Redirect::to(uri!(show_role_page(
					role.id,
					None::<String>,
					Some(format!(
						"Successfully assigned this role to user “{}”",
						user.username,
					)),
				))),
				json: Custom(Status::Ok, ()),
			}
		},
		Err(ZauthError::NotFound(_)) => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("User not found"),
				None::<String>,
			))),
			json: Custom(Status::NotFound, ()),
		},
		_ => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("An internal server error occured"),
				None::<String>,
			))),
			json: Custom(Status::InternalServerError, ()),
		},
	})
}

#[post("/roles/<role_id>/clients", data = "<client_id>")]
pub async fn add_client<'r>(
	// from url
	role_id: i32,
	// from body
	client_id: Form<i32>,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let client_result = Client::find(client_id.clone(), &db).await;
	Ok(match client_result {
		Ok(client) => {
			role.add_client(client.id, &db).await?;
			Accepter {
				html: Redirect::to(uri!(show_role_page(
					role.id,
					None::<String>,
					Some(format!(
						"Successfully assigned this role to client “{}”",
						client.name,
					)),
				))),
				json: Custom(Status::Ok, ()),
			}
		},
		Err(ZauthError::NotFound(_)) => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("Client not found"),
				None::<String>,
			))),
			json: Custom(Status::NotFound, ()),
		},
		_ => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("An internal server error occured"),
				None::<String>,
			))),
			json: Custom(Status::InternalServerError, ()),
		},
	})
}

#[delete("/roles/<role_id>/limited_to_clients/<client_id>")]
pub async fn delete_limited_to_client<'r>(
	// from url
	role_id: i32,
	client_id: i32,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let client = Client::find(client_id, &db).await?;
	role.remove_limited_to_client(client_id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_role_page(
			role_id,
			None::<String>,
			Some(format!(
				"Successfully removed client “{}” from the “limited to” list",
				client.name,
			))
		))),
		json: Custom(Status::Ok, ()),
	})
}

#[delete("/roles/<role_id>/users/<user_id>")]
pub async fn delete_user<'r>(
	// from url
	role_id: i32,
	user_id: i32,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let user = User::find(user_id, &db).await?;
	role.remove_user(user_id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_role_page(
			role_id,
			None::<String>,
			Some(format!(
				"Successfully removed this role from user “{}”",
				user.username,
			))
		))),
		json: Custom(Status::Ok, ()),
	})
}

#[delete("/roles/<role_id>/clients/<client_id>")]
pub async fn delete_client<'r>(
	// from url
	role_id: i32,
	client_id: i32,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let client = Client::find(client_id, &db).await?;
	role.remove_client(client_id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_role_page(
			role_id,
			None::<String>,
			Some(format!(
				"Successfully removed this role from client “{}”",
				client.name,
			))
		))),
		json: Custom(Status::Ok, ()),
	})
}

#[delete("/roles/<id>")]
pub async fn delete_role<'r>(
	// from url
	id: i32,
	// from headers
	_session: AdminSession,
	// injected
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(id, &db).await?;
	role.delete(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(list_roles(None::<String>))),
		json: Custom(Status::NoContent, ()),
	})
}
