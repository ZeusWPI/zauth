use rocket::form::Form;
use rocket::http::Status;
use rocket::response::status;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket::serde::json::Json;
use std::fmt::Debug;

use crate::DbConn;
use crate::ephemeral::from_api::Api;
use crate::ephemeral::from_api::SplitApi;
use crate::ephemeral::session::{AdminSession, ClientSession};
use crate::errors::Result;
use crate::models::client::*;
use crate::models::role::Role;
use crate::models::user::User;
use crate::views::accepter::Accepter;

// These structs need to be defined separately. Because we use a hidden field in
// the HTML to make sure we always get some value for `needs_grant`, we need
// Rocket to be lenient when parsing the form data. However, the `Lenient`
// struct does not play nice with any other libraries. (So it can't be
// deserialized by serde.)

#[derive(Deserialize, Debug)]
pub struct JsonClientChange {
	pub name: Option<String>,
	pub needs_grant: Option<bool>,
	pub description: Option<String>,
	pub redirect_uri_list: Option<String>,
}

#[derive(FromForm, Debug)]
pub struct FormClientChange {
	pub name: Option<String>,
	pub needs_grant: Vec<bool>,
	pub description: Option<String>,
	pub redirect_uri_list: Option<String>,
}

impl std::convert::From<JsonClientChange> for ClientChange {
	fn from(val: JsonClientChange) -> Self {
		ClientChange {
			name: val.name,
			needs_grant: val.needs_grant,
			description: val.description,
			redirect_uri_list: val.redirect_uri_list,
		}
	}
}

impl std::convert::From<FormClientChange> for ClientChange {
	fn from(val: FormClientChange) -> Self {
		ClientChange {
			name: val.name,
			needs_grant: val.needs_grant.last().cloned(),
			description: val.description,
			redirect_uri_list: val.redirect_uri_list,
		}
	}
}

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

	let roles = Role::all(&db).await?;

	Ok(template! { "clients/edit_client.html";
		current_user: User = session.admin,
		client: Client = client.clone(),
		client_roles: Vec<Role> = client.roles(&db).await?,
		roles: Vec<Role> = roles
	})
}

#[put("/clients/<id>", data = "<change>")]
pub async fn update_client<'r>(
	id: i32,
	change: SplitApi<FormClientChange, JsonClientChange, ClientChange>,
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

#[get("/clients/<id>/generate_secret")]
pub async fn get_generate_secret<'r>(
	id: i32,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let client = Client::find(id, &db).await?;
	Ok(template! { "clients/confirm_generate_secret.html";
		client: Client = client,
	})
}

#[post("/clients/<id>/generate_secret")]
pub async fn post_generate_secret<'r>(
	id: i32,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let client = Client::find(id, &db).await?;
	let client = client.generate_secret(&db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(update_client_page(client.id))),
		json: Custom(Status::NoContent, ()),
	})
}

#[derive(Serialize)]
pub struct ClientInfo {
	id: i32,
	name: String,
	roles: Vec<String>,
}

impl ClientInfo {
	async fn new(client: Client, db: &DbConn) -> Result<Self> {
		let roles = client
			.clone()
			.roles(db)
			.await?
			.iter()
			.map(|r| r.clone().name)
			.collect();

		Ok(ClientInfo {
			id: client.id,
			name: client.name,
			roles,
		})
	}
}

#[get("/current_client")]
pub async fn current_client(
	session: ClientSession,
	db: DbConn,
) -> Result<Json<ClientInfo>> {
	Ok(Json(ClientInfo::new(session.client, &db).await?))
}

#[post("/clients/<id>/roles", data = "<role_id>")]
pub async fn add_role<'r>(
	id: i32,
	role_id: Form<i32>,
	db: DbConn,
	_session: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(*role_id, &db).await?;
	let client = Client::find(id, &db).await?;
	role.add_client(client.id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(update_client_page(client.id))),
		json: Custom(Status::NoContent, ()),
	})
}

#[delete("/clients/<id>/roles/<role_id>")]
pub async fn delete_role<'r>(
	role_id: i32,
	id: i32,
	_session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let role = Role::find(role_id, &db).await?;
	let client = Client::find(id, &db).await?;
	role.remove_client(client.id, &db).await?;
	Ok(Accepter {
		html: Redirect::to(uri!(update_client_page(client.id))),
		json: Custom(Status::NoContent, ()),
	})
}
