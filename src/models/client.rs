use diesel::{self, prelude::*};

use crate::DbConn;
use crate::errors::{AuthenticationError, Result, ZauthError};

use crate::models::schema::{clients, roles};

use crate::util::random_token;
use chrono::NaiveDateTime;
use validator::Validate;

use super::role::Role;

const SECRET_LENGTH: usize = 64;

pub mod schema {
	table! {
		clients {
			id -> Integer,
			name -> Text,
			description -> Text,
			secret -> Text,
			needs_grant -> Bool,
			redirect_uri_list -> Text,
			created_at -> Timestamp,
		}
	}
}

#[derive(Serialize, AsChangeset, Queryable, Debug, Clone)]
pub struct Client {
	pub id: i32,
	pub name: String,
	pub description: String,
	pub secret: String,
	pub needs_grant: bool,
	pub redirect_uri_list: String,
	pub created_at: NaiveDateTime,
}

#[derive(Validate, FromForm, Deserialize, Debug, Clone)]
pub struct NewClient {
	#[validate(length(min = 3, max = 80))]
	pub name: String,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = clients)]
pub struct NewClientWithSecret {
	pub name: String,
	pub secret: String,
}

#[derive(Debug, Clone)]
pub struct ClientChange {
	pub name: Option<String>,
	pub needs_grant: Option<bool>,
	pub description: Option<String>,
	pub redirect_uri_list: Option<String>,
}

impl Client {
	pub async fn all(db: &DbConn) -> Result<Vec<Client>> {
		let all_clients = db
			.run(move |conn| clients::table.load::<Client>(conn))
			.await?;
		Ok(all_clients)
	}

	fn generate_random_secret() -> String {
		random_token(SECRET_LENGTH)
	}

	pub async fn create(client: NewClient, db: &DbConn) -> Result<Client> {
		client.validate()?;
		let client = NewClientWithSecret {
			name: client.name,
			secret: Self::generate_random_secret(),
		};
		db.run(move |conn| {
			conn.transaction(|conn| {
				// Create a new client
				diesel::insert_into(clients::table)
					.values(&client)
					.execute(conn)?;
				// Fetch the last created client
				clients::table.order(clients::id.desc()).first(conn)
			})
			.map_err(ZauthError::from)
		})
		.await
	}

	pub fn change_with(&mut self, change: ClientChange) -> Result<()> {
		if let Some(name) = change.name {
			self.name = name;
		}
		if let Some(needs_grant) = change.needs_grant {
			self.needs_grant = needs_grant;
		}
		if let Some(description) = change.description {
			self.description = description;
		}
		if let Some(redirect_uri_list) = change.redirect_uri_list {
			self.redirect_uri_list = redirect_uri_list
				.split_whitespace()
				.collect::<Vec<&str>>()
				.join("\n")
		}
		Ok(())
	}

	pub async fn update(self, db: &DbConn) -> Result<Self> {
		let id = self.id;
		db.run(move |conn| {
			conn.transaction(|conn| {
				// Update a client
				diesel::update(clients::table.find(id))
					.set(self)
					.execute(conn)?;

				// Fetch the updated record
				clients::table.find(id).first(conn)
			})
			.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn reload(self, db: &DbConn) -> Result<Self> {
		Self::find(self.id, db).await
	}

	pub async fn delete(self, db: &DbConn) -> Result<()> {
		db.run(move |conn| {
			diesel::delete(clients::table.find(self.id)).execute(conn)
		})
		.await?;
		Ok(())
	}

	pub async fn find_by_name(name: String, db: &DbConn) -> Result<Client> {
		let client = db
			.run(move |conn| {
				clients::table.filter(clients::name.eq(name)).first(conn)
			})
			.await?;
		Ok(client)
	}

	pub async fn find(id: i32, db: &DbConn) -> Result<Client> {
		let client = db
			.run(move |conn| clients::table.find(id).first(conn))
			.await?;
		Ok(client)
	}

	pub fn redirect_uri_acceptable(&self, redirect_uri: &str) -> bool {
		self.redirect_uri_list
			.lines()
			.any(|uri| uri == redirect_uri)
	}

	pub async fn find_and_authenticate(
		name: String,
		secret: &str,
		db: &DbConn,
	) -> Result<Client> {
		let client = Self::find_by_name(name, db).await?;
		if client.secret == secret {
			Ok(client)
		} else {
			Err(ZauthError::from(AuthenticationError::AuthFailed))
		}
	}

	pub async fn generate_secret(mut self, db: &DbConn) -> Result<Self> {
		self.secret = Self::generate_random_secret();
		self.update(db).await
	}

	pub async fn roles(&self, db: &DbConn) -> Result<Vec<Role>> {
		let id = self.id;
		db.run(move |conn| {
			roles::table
				.filter(roles::client_id.eq(id))
				.select(Role::as_select())
				.get_results(conn)
		})
		.await
		.map_err(ZauthError::from)
	}
}
