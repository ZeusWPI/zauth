use diesel::{self, prelude::*};

use self::schema::user;
use self::schema::user::dsl::user as users;
use crate::errors::{AuthenticationError, Result, ZauthError};
use crate::ConcreteConnection;

use pwhash::bcrypt::{self, BcryptSetup};

const DEFAULT_COST: u32 = 11;
const BCRYPT_SETUP: BcryptSetup = BcryptSetup {
	salt: None,
	variant: None,
	cost: Some(DEFAULT_COST),
};

mod schema {
	table! {
		user {
			id -> Integer,
			username -> Text,
			#[sql_name = "password"]
			hashed_password -> Text,
			admin -> Bool,
		}
	}
}

#[derive(Serialize, AsChangeset, Queryable, Debug, Clone)]
#[table_name = "user"]
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

#[table_name = "user"]
#[derive(Serialize, Insertable, Debug, Clone)]
struct NewUserHashed {
	username: String,
	hashed_password: String,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct UserChange {
	username: Option<String>,
	password: Option<String>,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct ChangeAdmin {
	pub admin: bool,
}

impl User {
	pub fn all(conn: &ConcreteConnection) -> Result<Vec<User>> {
		let all_users = users.load::<User>(conn)?;
		Ok(all_users)
	}

	pub fn create(user: NewUser, conn: &ConcreteConnection) -> Result<User> {
		let user = NewUserHashed {
			username: user.username,
			hashed_password: hash(&user.password)?,
		};
		conn.transaction(|| {
			// Create a new user
			diesel::insert_into(user::table)
				.values(&user)
				.execute(conn)?;
			// Fetch the last created user
			let user = users.order(user::id.desc()).first(conn)?;
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
			diesel::update(users.find(id)).set(self).execute(conn)?;
			// Fetch the updated record
			users.find(id).first(conn)
		})
		.map_err(|e| e.into())
	}

	pub fn find(id: i32, conn: &ConcreteConnection) -> Result<User> {
		users.find(id).first(conn).map_err(|e| e.into())
	}

	pub fn last(conn: &ConcreteConnection) -> Result<User> {
		users
			.order(user::id.desc())
			.first(conn)
			.map_err(|e| e.into())
	}

	pub fn find_and_authenticate(
		username: &str,
		password: &str,
		conn: &ConcreteConnection,
	) -> Result<User> {
		users
			.filter(user::username.eq(username))
			.first(conn)
			.map_err(ZauthError::from)
			.and_then(|user: User| {
				if verify(password, &user.hashed_password) {
					Ok(user)
				} else {
					Err(ZauthError::from(AuthenticationError::AuthFailed))
				}
			})
	}
}

fn hash(password: &str) -> crate::errors::EncodingResult<String> {
	let encrypted = bcrypt::hash_with(BCRYPT_SETUP, password)?;
	Ok(encrypted)
}

fn verify(password: &str, hash: &str) -> bool {
	bcrypt::verify(password, &hash)
}
