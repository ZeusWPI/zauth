use crate::errors::{Result, ZauthError};
use crate::models::client::schema::clients;
use crate::models::client::{Client, NewClient};
use crate::models::user::schema::users;
use crate::models::user::{NewUser, User};
use crate::util::random_token;
use crate::DbConn;
use diesel::RunQueryDsl;
use std::default::Default;

pub struct Seeder {
	empty_db: bool,
	clients_to_seed: usize,
	users_to_seed: usize,
	admin_password: Option<String>,
	client_name: Option<String>,
	client_redirect_uri: Option<String>,
}

impl Default for Seeder {
	fn default() -> Self {
		Seeder {
			empty_db: false,
			clients_to_seed: 0,
			users_to_seed: 0,
			admin_password: None,
			client_name: None,
			client_redirect_uri: None,
		}
	}
}

impl Seeder {
	pub fn from_env() -> Self {
		let mut seeder = Self::default();
		if let Ok(_) = std::env::var("ZAUTH_EMPTY_DB") {
			seeder.empty_db = true;
		}
		if let Ok(number) = std::env::var("ZAUTH_SEED_CLIENTS") {
			match number.parse() {
				Ok(num) => seeder.clients_to_seed = num,
				Err(_) => eprintln!(
					"ZAUTH_SEED_CLIENT=S\"{}\" error, expected number",
					number
				),
			};
		}
		if let Ok(number) = std::env::var("ZAUTH_SEED_USERS") {
			match number.parse() {
				Ok(num) => seeder.users_to_seed = num,
				Err(_) => eprintln!(
					"ZAUTH_SEED_USERS=\"{}\" error, expected number",
					number
				),
			};
		}
		if let Ok(pw) = std::env::var("ZAUTH_ADMIN_PASSWORD") {
			seeder.admin_password = Some(pw);
		}
		if let Ok(client) = std::env::var("ZAUTH_CLIENT_NAME") {
			seeder.client_name = Some(client);
		}
		if let Ok(uri) = std::env::var("ZAUTH_CLIENT_REDIRECT_URI") {
			seeder.client_redirect_uri = Some(uri);
		}
		seeder
	}

	pub async fn run(&self, bcrypt_cost: u32, db: &DbConn) -> Result<()> {
		if self.empty_db {
			self.delete_all(&db).await?;
		}
		if self.admin_password.is_some() {
			self.seed_admin(bcrypt_cost, &db).await?;
		}
		if self.client_name.is_some() {
			self.seed_client(&db).await?;
		}
		if self.clients_to_seed > 0 {
			self.seed_clients(&db).await?;
		}
		if self.users_to_seed > 0 {
			self.seed_users(bcrypt_cost, &db).await?;
		}
		Ok(())
	}

	async fn delete_all(&self, db: &DbConn) -> Result<()> {
		db.run(|conn| {
			diesel::delete(users::table)
				.execute(conn)
				.map_err(ZauthError::from)?;
			diesel::delete(clients::table)
				.execute(conn)
				.map_err(ZauthError::from)
		})
		.await?;
		eprintln!("Database cleared");
		Ok(())
	}

	async fn seed_clients(&self, db: &DbConn) -> Result<()> {
		for i in 1..self.clients_to_seed {
			let mut client = Client::create(
				NewClient {
					name: format!("Test client {}", i),
				},
				&db,
			)
			.await?;
			client.redirect_uri_list =
				format!("http://client{}.example.com/redirect/", i);
			client.needs_grant = i % 2 == 0;
			client.update(&db).await?;
		}
		eprintln!("Seeded {} clients", self.clients_to_seed);
		Ok(())
	}

	async fn seed_users(&self, bcrypt_cost: u32, db: &DbConn) -> Result<()> {
		for i in 1..self.users_to_seed {
			let new_user = NewUser {
				username: format!("user{}", i),
				password: random_token(12),
				full_name: format!("Test user {}", i),
				email: format!("user{}@example.com", i),
				ssh_key: None,
				not_a_robot: true,
			};
			User::create(new_user, bcrypt_cost, &db).await?;
		}
		eprintln!("Seeded {} users", self.users_to_seed);
		Ok(())
	}

	async fn seed_admin(&self, bcrypt_cost: u32, db: &DbConn) -> Result<()> {
		let username = String::from("admin");
		let password = self
			.admin_password
			.as_ref()
			.unwrap_or(&String::from("admin"))
			.clone();
		let admin = User::find_by_username(username.clone(), &db).await;
		if admin.is_err() {
			let mut admin = User::create(
				NewUser {
					username: username.clone(),
					password: password.clone(),
					full_name: String::from("Admin McAdmin"),
					email: String::from("admin@example.com"),
					ssh_key: None,
					not_a_robot: true,
				},
				bcrypt_cost,
				&db,
			)
			.await?;
			admin.admin = true;
			admin.update(&db).await?;
			eprintln!(
				"Seeded admin with username \"{}\" and password \"{}\"",
				username, password
			);
		}
		Ok(())
	}

	async fn seed_client(&self, db: &DbConn) -> Result<()> {
		let name = self.client_name.as_ref().expect("client name");
		let client = Client::find_by_name(name.clone(), &db).await;
		if client.is_err() {
			let mut client =
				Client::create(NewClient { name: name.clone() }, &db).await?;
			client.redirect_uri_list = self
				.client_redirect_uri
				.as_ref()
				.unwrap_or(&String::from(""))
				.to_string();
			client.update(&db).await?;
			eprintln!("Seeded client with name \"{}\"", name)
		}
		Ok(())
	}
}
