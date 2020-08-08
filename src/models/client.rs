use diesel::{self, prelude::*};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::errors::{AuthenticationError, Result, ZauthError};
use crate::ConcreteConnection;

use self::schema::clients;

const SECRET_LENGTH: usize = 64;

mod schema {
	table! {
		clients {
			id -> Integer,
			name -> Text,
			secret -> Text,
			needs_grant -> Bool,
			redirect_uri_list -> Text,
		}
	}
}

#[derive(Serialize, Queryable, Debug, Clone)]
pub struct Client {
	pub id: i32,
	pub name: String,
	pub secret: String,
	pub needs_grant: bool,
	pub redirect_uri_list: String,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct NewClient {
	pub name: String,
	pub needs_grant: bool,
	pub redirect_uri_list: String,
}

#[table_name = "clients"]
#[derive(Insertable, Debug, Clone)]
pub struct NewClientWithSecret {
	pub name: String,
	pub needs_grant: bool,
	pub secret: String,
	pub redirect_uri_list: String,
}

impl Client {
	pub fn all(conn: &ConcreteConnection) -> Result<Vec<Client>> {
		let all_clients = clients::table.load::<Client>(conn)?;
		Ok(all_clients)
	}

	fn generate_random_secret() -> String {
		thread_rng()
			.sample_iter(&Alphanumeric)
			.take(SECRET_LENGTH)
			.collect()
	}

	pub fn create(
		client: NewClient,
		conn: &ConcreteConnection,
	) -> Result<Client> {
		let client = NewClientWithSecret {
			name: client.name,
			needs_grant: client.needs_grant,
			redirect_uri_list: client.redirect_uri_list,
			secret: Self::generate_random_secret(),
		};
		dbg!(&client);
		let client = conn
			.transaction(|| {
				// Create a new user
				diesel::insert_into(clients::table)
					.values(&client)
					.execute(conn)?;
				// Fetch the last created user
				clients::table.order(clients::id.desc()).first(conn)
			})
			.map_err(ZauthError::from);
		dbg!(&client);
		return client;
	}

	pub fn find_by_name(
		name: &str,
		conn: &ConcreteConnection,
	) -> Result<Client> {
		let client =
			clients::table.filter(clients::name.eq(name)).first(conn)?;
		Ok(client)
	}

	pub fn find(id: i32, conn: &ConcreteConnection) -> Result<Client> {
		let client = clients::table.find(id).first(conn)?;
		Ok(client)
	}

	pub fn redirect_uri_acceptable(&self, redirect_uri: &str) -> bool {
		self.redirect_uri_list
			.split('\n')
			.any(|uri| uri == redirect_uri)
	}

	pub fn find_and_authenticate(
		name: &str,
		secret: &str,
		conn: &ConcreteConnection,
	) -> Result<Client> {
		let client = Self::find_by_name(name, conn)?;
		if client.secret == secret {
			Ok(client)
		} else {
			Err(ZauthError::from(AuthenticationError::AuthFailed))
		}
	}
}
