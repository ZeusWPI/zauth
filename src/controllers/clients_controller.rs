use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket::serde::json::Json;
use std::fmt::Debug;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::AdminSession;
use crate::errors::Result;
use crate::models::client::*;
use crate::models::user::User;
use crate::views::accepter::Accepter;
use crate::DbConn;

#[get("/clients")]
pub async fn list_clients<'r>(
	db: DbConn,
	session: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	let clients = Client::all(&db).await?;
	Ok(Accepter {
		html: template! {
			"clients/index.html";
			clients: Vec<Client> = clients.clone(),
			current_user: User = session.admin,
		},
		json: Json(clients),
	})
}

#[get("/clients/<id>/edit")]
pub async fn update_client_page<'r>(
	id: i32,
	session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let client = Client::find(id, &db).await?;

	Ok(template! { "clients/edit_client.html";
		current_user: User = session.admin,
		client: Client = client,
	})
}

#[put("/clients/<id>", data = "<change>")]
pub async fn update_client<'r>(
	id: i32,
	change: Api<ClientChange>,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mut client = Client::find(id, &db).await?;
	client.change_with(change.into_inner())?;
	client.update(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(list_clients)),
		json: Custom(Status::NoContent, ()),
	})
}

#[delete("/clients/<id>")]
pub async fn delete_client<'r>(
	id: i32,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let client = Client::find(id, &db).await?;
	client.delete(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(list_clients)),
		json: Custom(Status::NoContent, ()),
	})
}

#[post("/clients", data = "<client>")]
pub async fn create_client<'r>(
	client: Api<NewClient>,
	db: DbConn,
	_admin: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	let client = Client::create(client.into_inner(), &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(update_client_page(client.id))),
		json: status::Created::new(String::from("/client")).body(Json(client)),
	})
}
