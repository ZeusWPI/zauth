use self::schema::users;
use crate::errors::{Result, ZauthError};
use crate::ConcreteConnection;
use diesel::{self, prelude::*};

use pwhash::bcrypt::{self, BcryptSetup};

const DEFAULT_COST: u32 = 11;
const BCRYPT_SETUP: BcryptSetup = BcryptSetup {
	salt: None,
	variant: None,
	cost: Some(DEFAULT_COST),
};

mod schema {
	table! {
		users {
			id -> Integer,
			username -> Text,
			#[sql_name = "password"]
			hashed_password -> Text,
			admin -> Bool,
		}
	}
}

#[derive(Serialize, AsChangeset, Queryable, Debug, Clone)]
#[table_name = "users"]
pub struct User {
	pub id: i32,
	pub username: String,
	#[serde(skip)] // Let's not send our users their hashed password, shall we?
	pub hashed_password: String,
	pub admin: bool,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct NewUser {
	pub username: String,
	pub password: String,
}

#[table_name = "users"]
#[derive(Serialize, Insertable, Debug, Clone)]
struct NewUserHashed {
	username: String,
	hashed_password: String,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct UserChange {
	pub username: Option<String>,
	pub password: Option<String>,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct ChangeAdmin {
	pub admin: bool,
}

impl User {
	pub fn all(conn: &ConcreteConnection) -> Result<Vec<User>> {
		let all_users = users::table.load::<User>(conn)?;
		Ok(all_users)
	}

	pub fn find_by_username(
		username: &str,
		conn: &ConcreteConnection,
	) -> diesel::result::QueryResult<User> {
		users::table
			.filter(users::username.eq(username))
			.first(conn)
		// .map_err(ZauthError::from)
	}

	pub fn create(user: NewUser, conn: &ConcreteConnection) -> Result<User> {
		let user = NewUserHashed {
			username: user.username,
			hashed_password: hash(&user.password)?,
		};
		conn.transaction(|| {
			// Create a new user
			diesel::insert_into(users::table)
				.values(&user)
				.execute(conn)?;
			// Fetch the last created user
			let user = users::table.order(users::id.desc()).first(conn)?;
			Ok(user)
		})
	}

	pub fn change_with(&mut self, change: UserChange) -> Result<()> {
		if let Some(username) = change.username {
			self.username = username;
		}
		if let Some(password) = change.password {
			self.hashed_password = hash(&password)?;
		}
		Ok(())
	}

	pub fn update(self, conn: &ConcreteConnection) -> Result<Self> {
		let id = self.id;
		conn.transaction(|| {
			// Create a new user
			diesel::update(users::table.find(id))
				.set(self)
				.execute(conn)?;
			// Fetch the updated record
			users::table.find(id).first(conn)
		})
		.map_err(ZauthError::from)
	}

	pub fn find(id: i32, conn: &ConcreteConnection) -> Result<User> {
		users::table.find(id).first(conn).map_err(ZauthError::from)
	}

	pub fn last(conn: &ConcreteConnection) -> Result<User> {
		users::table
			.order(users::id.desc())
			.first(conn)
			.map_err(ZauthError::from)
	}

	pub fn find_and_authenticate(
		username: &str,
		password: &str,
		conn: &ConcreteConnection,
	) -> Result<Option<User>> {
		match Self::find_by_username(username, conn) {
			Ok(user) if !verify(password, &user.hashed_password) => Ok(None),
			Ok(user) => Ok(Some(user)),
			Err(diesel::result::Error::NotFound) => Ok(None),
			Err(e) => Err(ZauthError::from(e)),
		}
	}
}

fn hash(password: &str) -> crate::errors::InternalResult<String> {
	let hashed = bcrypt::hash_with(BCRYPT_SETUP, password)?;
	Ok(hashed)
}

fn verify(password: &str, hash: &str) -> bool {
	bcrypt::verify(password, &hash)
}
