use rocket::response::status;
use rocket_contrib::json::Json;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::AdminSession;
use crate::errors::Result;
use crate::models::client::*;
use crate::DbConn;

#[get("/clients")]
pub fn list_clients(
	conn: DbConn,
	_admin: AdminSession,
) -> Result<Json<Vec<Client>>> {
	let clients = Client::all(&conn)?;
	Ok(Json(clients))
}

#[post("/clients", data = "<client>")]
pub fn create_client(
	client: Api<NewClient>,
	conn: DbConn,
	_admin: AdminSession,
) -> status::Created<Json<Option<Client>>> {
	status::Created(
		String::from("/"),
		Some(Json(Client::create(client.into_inner(), &conn))),
	)
}
