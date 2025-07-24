use diesel::result::DatabaseErrorKind;
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder, status};
use rocket::serde::json::Json;
use std::fmt::Debug;

use crate::DbConn;
use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::AdminSession;
use crate::errors::{Either, InternalError, Result, ZauthError};
use crate::models::client::Client;
use crate::models::role::{NewRole, Role};
use crate::models::user::User;
use crate::views::accepter::Accepter;

#[get("/roles?<error>")]
pub async fn list_roles<'r>(
	error: Option<String>,
	db: DbConn,
	session: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	let roles = Role::all(&db).await?;
	let clients = Client::all(&db).await?;

	Ok(Accepter {
		html: template! {
			"roles/index.html";
			roles: Vec<Role> = roles.clone(),
			clients: Vec<Client> = clients,
			error: Option<String> = error,
			current_user: User = session.admin,
		},
		json: Json(roles),
	})
}

#[post("/roles", data = "<role>")]
pub async fn create_role<'r, 'a>(
	role: Api<NewRole>,
	db: DbConn,
	_admin: AdminSession,
) -> Result<
	Either<impl Responder<'a, 'static>, impl Responder<'r, 'static> + use<'r>>,
> {
	let role = Role::create(role.into_inner(), &db).await;
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
		))) => Ok(Either::Right(Accepter {
			html: Redirect::to(uri!(list_roles(Some(
				"role name already exists"
			)))),
			json: "role name already exists",
		})),
		Err(err) => Err(err),
	}
}

#[get("/roles/<id>?<error>&<info>")]
pub async fn show_role_page<'r>(
	id: i32,
	error: Option<String>,
	info: Option<String>,
	session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(id, &db).await?;
	let users = role.clone().users(&db).await?;

	let client = if let Some(id) = role.client_id {
		Some(Client::find(id, &db).await?)
	} else {
		None
	};

	Ok(template! { "roles/show_role.html";
		current_user: User = session.admin,
		role: Role = role,
		client: Option<Client> = client,
		users: Vec<User> = users,
		error: Option<String> = error,
		info: Option<String> = info
	})
}

#[delete("/roles/<id>")]
pub async fn delete_role<'r>(
	id: i32,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(id, &db).await?;
	role.delete(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(list_roles(None::<String>))),
		json: Custom(Status::NoContent, ()),
	})
}

#[post("/roles/<role_id>/users", data = "<username>")]
pub async fn add_user<'r>(
	username: Form<String>,
	role_id: i32,
	db: DbConn,
	_session: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let user_result = User::find_by_username(username.clone(), &db).await;
	Ok(match user_result {
		Ok(user) => {
			role.add_user(user.id, &db).await?;
			Accepter {
				html: Redirect::to(uri!(show_role_page(
					role.id,
					None::<String>,
					Some("user added")
				))),
				json: Custom(Status::Ok, ()),
			}
		},
		Err(ZauthError::NotFound(_)) => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("user not found"),
				None::<String>
			))),
			json: Custom(Status::NotFound, ()),
		},
		_ => Accepter {
			html: Redirect::to(uri!(show_role_page(
				role.id,
				Some("error occured"),
				None::<String>
			))),
			json: Custom(Status::InternalServerError, ()),
		},
	})
}

#[delete("/roles/<role_id>/users/<user_id>")]
pub async fn delete_user<'r>(
	role_id: i32,
	user_id: i32,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	role.remove_user(user_id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(show_role_page(
			role_id,
			None::<String>,
			Some("user deleted")
		))),
		json: Custom(Status::Ok, ()),
	})
}
