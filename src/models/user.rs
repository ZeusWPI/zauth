use self::schema::users;
use crate::errors::{InternalError, LoginError, Result, ZauthError};
use crate::ConcreteConnection;
use diesel::{self, prelude::*};
use diesel_derive_enum::DbEnum;
use std::fmt;

use crate::models::user::UserState::{Active, PendingApproval};
use chrono::{NaiveDateTime, Utc};
use lettre::message::Mailbox;
use pwhash::bcrypt::{self, BcryptSetup};
use regex::Regex;
use std::convert::TryInto;
use validator::{Validate, ValidationError};

#[derive(DbEnum, Debug, Serialize, Clone, PartialEq)]
pub enum UserState {
	PendingApproval,
	PendingMailConfirmation,
	Active,
	Disabled,
}

impl fmt::Display for UserState {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			UserState::PendingApproval => {
				write!(f, "Admin approval pending")
			},
			UserState::PendingMailConfirmation => {
				write!(f, "Email confirmation pending")
			},
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
			password_reset_token -> Nullable<Varchar>,
			password_reset_expiry -> Nullable<Timestamp>,
			full_name -> Varchar,
			email -> Varchar,
			ssh_key -> Nullable<Text>,
			state -> UserStateMapping,
			last_login -> Timestamp,
			created_at -> Timestamp,
		}
	}
}

#[derive(Validate, Serialize, AsChangeset, Queryable, Debug, Clone)]
#[table_name = "users"]
#[changeset_options(treat_none_as_null = "true")]
pub struct User {
	pub id:                    i32,
	#[validate(length(min = 3, max = 254))]
	pub username:              String,
	#[serde(skip)] // Let's not send our users their hashed password, shall we?
	pub hashed_password:       String,
	pub admin:                 bool,
	pub password_reset_token:  Option<String>,
	pub password_reset_expiry: Option<NaiveDateTime>,
	#[validate(length(min = 3, max = 254))]
	pub full_name:             String,
	#[validate(email)]
	pub email:                 String,
	#[validate(custom = "validate_ssh_key_list")]
	pub ssh_key:               Option<String>,
	pub state:                 UserState,
	pub last_login:            NaiveDateTime,
	pub created_at:            NaiveDateTime,
}

#[derive(Validate, FromForm, Deserialize, Debug, Clone)]
pub struct NewUser {
	#[validate(length(min = 3, max = 254))]
	pub username:  String,
	#[validate(length(min = 8))]
	pub password:  String,
	#[validate(length(min = 3, max = 254))]
	pub full_name: String,
	#[validate(email)]
	pub email:     String,
	#[validate(custom = "validate_ssh_key_list")]
	pub ssh_key:   Option<String>,
}

#[derive(Serialize, Insertable, Debug, Clone)]
#[table_name = "users"]
struct NewUserHashed {
	username:        String,
	hashed_password: String,
	full_name:       String,
	email:           String,
	state:           UserState,
	ssh_key:         Option<String>,
	last_login:      NaiveDateTime,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct UserChange {
	pub username:  Option<String>,
	pub password:  Option<String>,
	pub full_name: Option<String>,
	pub email:     Option<String>,
	pub ssh_key:   Option<String>,
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

	pub fn is_active(&self) -> bool {
		matches!(self.state, Active)
	}

	pub fn find_by_username(
		username: &str,
		conn: &ConcreteConnection,
	) -> Result<User> {
		users::table
			.filter(users::username.eq(username))
			.first(conn)
			.map_err(ZauthError::from)
	}

	pub fn find_by_email(
		email: &str,
		conn: &ConcreteConnection,
	) -> Result<User> {
		users::table
			.filter(users::email.eq(email))
			.first(conn)
			.map_err(ZauthError::from)
	}

	pub fn delete(self, conn: &ConcreteConnection) -> Result<()> {
		diesel::delete(users::table.find(self.id))
			.execute(conn)
			.map_err(ZauthError::from)?;
		Ok(())
	}

	pub fn find_by_token(
		token: &str,
		conn: &ConcreteConnection,
	) -> Result<Option<User>> {
		match users::table
			.filter(users::password_reset_token.eq(token))
			.first::<Self>(conn)
			.map_err(ZauthError::from)
		{
			Ok(user)
				if Utc::now().naive_utc()
					< user.password_reset_expiry.unwrap() =>
			{
				Ok(Some(user))
			},
			Ok(_) => Ok(None),
			Err(ZauthError::NotFound(_)) => Ok(None),
			Err(err) => Err(err),
		}
	}

	pub fn find_by_pending(conn: &ConcreteConnection) -> Result<Vec<User>> {
		let pending_users = users::table
			.filter(users::state.eq(UserState::PendingApproval))
			.load::<User>(conn)?;
		Ok(pending_users)
	}

	pub fn create(
		user: NewUser,
		bcrypt_cost: u32,
		conn: &ConcreteConnection,
	) -> Result<User> {
		user.validate()?;
		let user = NewUserHashed {
			username:        user.username,
			hashed_password: hash(&user.password, bcrypt_cost)?,
			full_name:       user.full_name,
			email:           user.email,
			ssh_key:         user.ssh_key,
			state:           Active,
			last_login:      Utc::now().naive_utc(),
		};
		Self::insert(user, conn)
	}

	pub fn create_pending(
		user: NewUser,
		bcrypt_cost: u32,
		conn: &ConcreteConnection,
	) -> Result<User> {
		user.validate()?;
		let user = NewUserHashed {
			username:        user.username,
			hashed_password: hash(&user.password, bcrypt_cost)?,
			full_name:       user.full_name,
			email:           user.email,
			ssh_key:         user.ssh_key,
			state:           PendingApproval,
			last_login:      Utc::now().naive_utc(),
		};
		Self::insert(user, conn)
	}

	fn insert(user: NewUserHashed, conn: &ConcreteConnection) -> Result<User> {
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

	pub fn change_with(
		&mut self,
		change: UserChange,
		bcrypt_cost: u32,
	) -> Result<()> {
		if let Some(username) = change.username {
			self.username = username;
		}
		if let Some(password) = change.password {
			self.hashed_password = hash(&password, bcrypt_cost)?;
		}
		if let Some(full_name) = change.full_name {
			self.full_name = full_name;
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

	pub fn change_password(
		mut self,
		new_password: &str,
		bcrypt_cost: u32,
		conn: &ConcreteConnection,
	) -> Result<Self> {
		self.hashed_password = hash(new_password, bcrypt_cost)?;
		self.password_reset_token = None;
		self.password_reset_expiry = None;
		self.update(conn)
	}

	pub fn reload(self, conn: &ConcreteConnection) -> Result<User> {
		Self::find(self.id, conn)
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
	) -> Result<User> {
		match Self::find_by_username(username, conn) {
			Ok(user) if !verify(password, &user.hashed_password) => {
				Err(ZauthError::LoginError(LoginError::UsernamePasswordError))
			},
			Ok(user) if user.state == UserState::PendingApproval => Err(
				ZauthError::LoginError(LoginError::AccountPendingApprovalError),
			),
			Ok(user) if user.state == UserState::PendingMailConfirmation => {
				Err(ZauthError::LoginError(
					LoginError::AccountPendingMailConfirmationError,
				))
			},
			Ok(user) if user.state == UserState::Disabled => {
				Err(ZauthError::LoginError(LoginError::AccountDisabledError))
			},
			Ok(user) => Ok(user),
			Err(ZauthError::NotFound(_msg)) => {
				Err(ZauthError::LoginError(LoginError::UsernamePasswordError))
			},
			Err(e) => Err(ZauthError::from(e)),
		}
	}
}

fn hash(
	password: &str,
	bcrypt_cost: u32,
) -> crate::errors::InternalResult<String> {
	let b: BcryptSetup = BcryptSetup {
		salt:    None,
		variant: None,
		cost:    Some(bcrypt_cost),
	};
	let hashed = bcrypt::hash_with(b, password)?;
	Ok(hashed)
}

fn verify(password: &str, hash: &str) -> bool {
	bcrypt::verify(password, &hash)
}

impl TryInto<Mailbox> for &User {
	type Error = ZauthError;

	fn try_into(self) -> Result<Mailbox> {
		Ok(Mailbox::new(
			Some(self.username.to_string()),
			self.email.clone().parse().map_err(InternalError::from)?,
		))
	}
}

fn validate_ssh_key_list(
	ssh_keys: &String,
) -> std::result::Result<(), ValidationError> {
	lazy_static! {
		static ref SSH_KEY_REGEX: Regex = Regex::new(
			r"ssh-(rsa|dsa|ecdsa|ed25519) [a-zA-Z0-9+/]{1,750}={0,3}( [^ ]+)?"
		)
		.unwrap();
	}
	for line in ssh_keys.lines() {
		let line = line.trim();
		if !line.is_empty() && !SSH_KEY_REGEX.is_match(line) {
			return Err(ValidationError::new("Invalid ssh key"));
		}
	}
	Ok(())
}
