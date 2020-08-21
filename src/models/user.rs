use self::schema::users;
use crate::errors::{Result, ZauthError};
use crate::ConcreteConnection;
use diesel::{self, prelude::*};
use diesel_derive_enum::DbEnum;
use std::fmt;

use chrono::{NaiveDateTime, Utc};
use lettre_email::Mailbox;
use pwhash::bcrypt::{self, BcryptSetup};

const DEFAULT_COST: u32 = 11;
const BCRYPT_SETUP: BcryptSetup = BcryptSetup {
	salt:    None,
	variant: None,
	cost:    Some(DEFAULT_COST),
};

#[derive(DbEnum, Debug, Serialize, Clone)]
pub enum UserState {
	Pending,
	Active,
	Disabled,
}

impl fmt::Display for UserState {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			UserState::Pending => write!(f, "Pending"),
			UserState::Active => write!(f, "Active"),
			UserState::Disabled => write!(f, "Disabled"),
		}
	}
}

mod schema {
	table! {
		use diesel::sql_types::*;
		use crate::models::user::UserStateMapping;

		users {
		id -> Integer,
		username -> Varchar,
		hashed_password -> Varchar,
		admin -> Bool,
		first_name -> Varchar,
		last_name -> Varchar,
		email -> Varchar,
		ssh_key -> Nullable<Text>,
		state -> UserStateMapping,
		last_login -> Timestamp,
		created_at -> Timestamp,
	}
	}
}

#[derive(Serialize, AsChangeset, Queryable, Debug, Clone)]
#[table_name = "users"]
pub struct User {
	pub id:              i32,
	// validate to have at least 3 chars
	pub username:        String,
	#[serde(skip)] // Let's not send our users their hashed password, shall we?
	pub hashed_password: String,
	pub admin:           bool,

	pub first_name: String,
	pub last_name:  String,
	pub email:      String,
	pub ssh_key:    Option<String>,
	pub state:      UserState,
	pub last_login: NaiveDateTime,
	pub created_at: NaiveDateTime,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct NewUser {
	pub username:   String,
	pub password:   String,
	pub first_name: String,
	pub last_name:  String,
	pub email:      String,
	pub ssh_key:    Option<String>,
}

#[table_name = "users"]
#[derive(Serialize, Insertable, Debug, Clone)]
struct NewUserHashed {
	username:        String,
	hashed_password: String,
	first_name:      String,
	last_name:       String,
	email:           String,
	ssh_key:         Option<String>,
	last_login:      NaiveDateTime,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct UserChange {
	pub username:   Option<String>,
	pub password:   Option<String>,
	pub first_name: Option<String>,
	pub last_name:  Option<String>,
	pub email:      Option<String>,
	pub ssh_key:    Option<String>,
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
	) -> diesel::result::QueryResult<User>
	{
		users::table
			.filter(users::username.eq(username))
			.first(conn)
		// .map_err(ZauthError::from)
	}

	pub fn create(user: NewUser, conn: &ConcreteConnection) -> Result<User> {
		let user = NewUserHashed {
			username:        user.username,
			hashed_password: hash(&user.password)?,
			first_name:      user.first_name,
			last_name:       user.last_name,
			email:           user.email,
			ssh_key:         user.ssh_key,
			last_login:      Utc::now().naive_utc(),
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
		if let Some(first_name) = change.first_name {
			self.first_name = first_name;
		}
		if let Some(last_name) = change.last_name {
			self.last_name = last_name;
		}
		if let Some(email) = change.email {
			self.email = email;
		}
		if let Some(ssh_key) = change.ssh_key {
			self.ssh_key = Some(ssh_key);
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
	) -> Result<Option<User>>
	{
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

impl Into<Mailbox> for &User {
	fn into(self) -> Mailbox {
		// TODO: user email
		Mailbox::new_with_name(
			self.username.to_string(),
			self.username.to_string(),
		)
	}
}
