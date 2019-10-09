use rocket::request::Form;
use rocket_contrib::json::Json;

use ephemeral::session::AdminSession;
use models::client::*;
use DbConn;

#[get("/clients")]
pub fn list_clients(conn: DbConn, _admin: AdminSession) -> Json<Vec<Client>> {
	Json(Client::all(&conn))
}

#[post("/clients", data = "<client>")]
pub fn create_client(
	client: Form<NewClient>,
	conn: DbConn,
	_admin: AdminSession,
) -> Json<Option<Client>>
{
	Json(Client::create(client.into_inner(), &conn))
}
