use rocket::response::status;
use rocket_contrib::json::Json;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::AdminSession;
use crate::models::client::*;
use crate::DbConn;

#[get("/clients")]
pub fn list_clients(conn: DbConn, _admin: AdminSession) -> Json<Vec<Client>> {
	Json(Client::all(&conn))
}

#[post("/clients", data = "<client>")]
pub fn create_client(
	client: Api<NewClient>,
	conn: DbConn,
	_admin: AdminSession,
) -> status::Created<Json<Option<Client>>>
{
	status::Created(
		String::from("/"),
		Some(Json(Client::create(client.into_inner(), &conn))),
	)
}
