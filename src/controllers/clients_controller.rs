use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket_contrib::json::Json;
use std::fmt::Debug;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::AdminSession;
use crate::errors::Result;
use crate::models::client::*;
use crate::models::user::User;
use crate::views::accepter::Accepter;
use crate::DbConn;

#[get("/clients")]
pub fn list_clients(
	conn: DbConn,
	session: AdminSession,
) -> Result<impl Responder<'static>> {
	let clients = Client::all(&conn)?;
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
pub fn update_client_page(
	id: i32,
	session: AdminSession,
	conn: DbConn,
) -> Result<impl Responder<'static>> {
	let client = Client::find(id, &conn)?;

	Ok(template! { "clients/edit_client.html";
		current_user: User = session.admin,
		client: Client = client,
	})
}

#[put("/clients/<id>", data = "<change>")]
pub fn update_client(
	id: i32,
	change: Api<ClientChange>,
	_session: AdminSession,
	conn: DbConn,
) -> Result<impl Responder<'static>> {
	let mut client = Client::find(id, &conn)?;
	client.change_with(change.into_inner())?;
	let _client = client.update(&conn)?;
	Ok(Accepter {
		html: Redirect::to(uri!(list_clients)),
		json: Custom(Status::NoContent, ()),
	})
}

#[delete("/clients/<id>")]
pub fn delete_client(
	id: i32,
	_session: AdminSession,
	conn: DbConn,
) -> Result<impl Responder<'static>> {
	let mut client = Client::find(id, &conn)?;
	client.delete(&conn)?;
	Ok(Accepter {
		html: Redirect::to(uri!(list_clients)),
		json: Custom(Status::NoContent, ()),
	})
}

#[post("/clients", data = "<client>")]
pub fn create_client(
	client: Api<NewClient>,
	conn: DbConn,
	_admin: AdminSession,
) -> Result<impl Responder<'static>> {
	let client = Client::create(client.into_inner(), &conn)?;
	Ok(Accepter {
		html: Redirect::to(uri!(update_client_page: client.id)),
		json: status::Created(String::from("/client"), Some(Json(client))),
	})
}
