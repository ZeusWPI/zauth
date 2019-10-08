use diesel::{self, prelude::*};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use self::schema::client;
use self::schema::client::dsl::client as clients;

const SECRET_LENGTH: usize = 64;

mod schema {
	table! {
		client {
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
	pub id:                i32,
	pub name:              String,
	pub secret:            String,
	pub needs_grant:       bool,
	pub redirect_uri_list: String,
}

#[derive(FromForm, Debug, Clone)]
pub struct NewClient {
	pub name:              String,
	pub needs_grant:       bool,
	pub redirect_uri_list: String,
}

#[table_name = "client"]
#[derive(Insertable, Debug, Clone)]
pub struct NewClientWithSecret {
	pub name:              String,
	pub needs_grant:       bool,
	pub secret:            String,
	pub redirect_uri_list: String,
}

impl Client {
	pub fn all(conn: &SqliteConnection) -> Vec<Client> {
		clients.load::<Client>(conn).unwrap()
	}

	fn generate_random_secret() -> String {
		thread_rng()
			.sample_iter(&Alphanumeric)
			.take(SECRET_LENGTH)
			.collect()
	}

	pub fn create(
		client: NewClient,
		conn: &SqliteConnection,
	) -> Option<Client>
	{
		let client = NewClientWithSecret {
			name:              client.name,
			needs_grant:       client.needs_grant,
			redirect_uri_list: client.redirect_uri_list,
			secret:            Self::generate_random_secret(),
		};
		conn.transaction(|| {
			// Create a new user
			diesel::insert_into(client::table)
				.values(&client)
				.execute(conn)?;
			// Fetch the last created user
			clients.order(client::id.desc()).first(conn)
		})
		.ok()
	}

	pub fn find_by_name(name: &str, conn: &SqliteConnection) -> Option<Client> {
		clients.filter(client::name.eq(name)).first(conn).ok()
	}

	pub fn find(id: i32, conn: &SqliteConnection) -> Option<Client> {
		clients.find(id).first(conn).ok()
	}

	pub fn redirect_uri_acceptable(&self, redirect_uri: &str) -> bool {
		self.redirect_uri_list
			.split('\n')
			.any(|uri| uri == redirect_uri)
	}

	pub fn find_and_authenticate(
		name: &str,
		secret: &str,
		conn: &SqliteConnection,
	) -> Option<Client>
	{
		Self::find_by_name(name, conn).filter(|client| client.secret == secret)
	}
}
