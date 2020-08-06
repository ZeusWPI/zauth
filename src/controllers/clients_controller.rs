use rocket::response::status;
use rocket::response::{Redirect, Responder};
use rocket_contrib::json::Json;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::{AdminSession, UserSession};
use crate::models::client::*;
use crate::models::user::User;
use crate::views::accepter::Accepter;
use crate::DbConn;

#[get("/clients")]
pub fn list_clients(conn: DbConn, session: AdminSession) -> impl Responder<'static> {
	let clients = Client::all(&conn);
	Accepter {
		html: template! {
			"clients/index.html";
			clients: Vec<Client> = clients.clone(),
			current_user: User = session.admin,
		},
		json: Json(clients),
	}
}

#[post("/clients", data = "<client>")]
pub fn create_client(
	client: Api<NewClient>,
	conn: DbConn,
	_admin: AdminSession,
) -> Option<impl Responder<'static>>
{
	println!("{:?}", client);
	let client = Client::create(client.into_inner(), &conn)?;
	Some(Accepter{
		html: Redirect::to(uri!(list_clients)),
		json: Json(client),
	})
}
