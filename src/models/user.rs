use self::schema::users;
use crate::errors::{InternalError, LoginError, Result, ZauthError};
use crate::DbConn;
use diesel::{self, prelude::*};
use diesel_derive_enum::DbEnum;
use std::fmt;

use crate::models::user::UserState::{Active, PendingApproval};
use chrono::{NaiveDateTime, Utc};
use lettre::message::Mailbox;
use pwhash::bcrypt::{self, BcryptSetup};
use regex::Regex;
use rocket::serde::Serialize;
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

pub mod schema {
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
#[serde(crate = "rocket::serde")]
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
	pub username:    String,
	#[validate(length(min = 8))]
	pub password:    String,
	#[validate(length(min = 3, max = 254))]
	pub full_name:   String,
	#[validate(email)]
	pub email:       String,
	#[validate(custom = "validate_ssh_key_list")]
	pub ssh_key:     Option<String>,
	#[validate(custom(function = "validate_not_a_robot"))]
	#[serde(default = "const_false")]
	pub not_a_robot: bool,
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
	pub async fn all(db: &DbConn) -> Result<Vec<User>> {
		let all_users =
			db.run(move |conn| users::table.load::<User>(conn)).await?;
		Ok(all_users)
	}

	pub fn is_active(&self) -> bool {
		matches!(self.state, Active)
	}

	pub async fn find_by_username<'r>(
		username: String,
		db: &DbConn,
	) -> Result<User> {
		db.run(move |conn| {
			users::table
				.filter(users::username.eq(username))
				.first(conn)
				.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn find_by_email(email: String, db: &DbConn) -> Result<User> {
		let query = users::table.filter(users::email.eq(email));
		db.run(move |conn| query.first(conn).map_err(ZauthError::from))
			.await
	}

	pub async fn delete(self, db: &DbConn) -> Result<()> {
		db.run(move |conn| {
			diesel::delete(users::table.find(self.id))
				.execute(conn)
				.map_err(ZauthError::from)
		})
		.await?;
		Ok(())
	}

	pub async fn find_by_token<'r>(
		token: String,
		db: &DbConn,
	) -> Result<Option<User>> {
		let token = token.to_owned();
		let result = db
			.run(move |conn| {
				users::table
					.filter(users::password_reset_token.eq(token))
					.first::<Self>(conn)
			})
			.await
			.map_err(ZauthError::from);
		match result {
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

	pub async fn find_by_pending(db: &DbConn) -> Result<Vec<User>> {
		let pending_users = db
			.run(move |conn| {
				users::table
					.filter(users::state.eq(UserState::PendingApproval))
					.load::<User>(conn)
			})
			.await?;
		Ok(pending_users)
	}

	pub async fn create(
		user: NewUser,
		bcrypt_cost: u32,
		db: &DbConn,
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
		Self::insert(user, db).await
	}

	pub async fn create_pending(
		user: NewUser,
		bcrypt_cost: u32,
		db: &DbConn,
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
		Self::insert(user, db).await
	}

	async fn insert(user: NewUserHashed, db: &DbConn) -> Result<User> {
		db.run(move |conn| {
			conn.transaction(|| {
				// Create a new user
				diesel::insert_into(users::table)
					.values(&user)
					.execute(conn)?;
				// Fetch the last created user
				let user = users::table.order(users::id.desc()).first(conn)?;
				Ok(user)
			})
		})
		.await
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

	pub async fn update(self, db: &DbConn) -> Result<Self> {
		let id = self.id;
		db.run(move |conn| {
			conn.transaction(|| {
				// Create a new user
				diesel::update(users::table.find(id))
					.set(self)
					.execute(conn)?;

				// Fetch the updated record
				users::table.find(id).first(conn)
			})
		})
		.await
		.map_err(ZauthError::from)
	}

	pub async fn change_password(
		mut self,
		new_password: &str,
		bcrypt_cost: u32,
		db: &DbConn,
	) -> Result<Self> {
		self.hashed_password = hash(new_password, bcrypt_cost)?;
		self.password_reset_token = None;
		self.password_reset_expiry = None;
		self.update(db).await
	}

	pub async fn reload(self, db: &DbConn) -> Result<User> {
		Self::find(self.id, db).await
	}

	pub async fn find(id: i32, db: &DbConn) -> Result<User> {
		db.run(move |conn| {
			users::table.find(id).first(conn).map_err(ZauthError::from)
		})
		.await
	}

	pub async fn last(db: &DbConn) -> Result<User> {
		db.run(move |conn| {
			users::table
				.order(users::id.desc())
				.first(conn)
				.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn find_and_authenticate(
		username: String,
		password: String,
		db: &DbConn,
	) -> Result<User> {
		match Self::find_by_username(username, db).await {
			Ok(user) if !verify(&password, &user.hashed_password) => {
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

fn validate_not_a_robot(
	not_a_robot: &bool,
) -> std::result::Result<(), ValidationError> {
	if !not_a_robot {
		return Err(ValidationError::new(
			"Non-human registration is currently not supported by the digital \
			 interface. Please interface with an aidmin.",
		));
	}
	Ok(())
}

/// used as a default for not_a_robot field
fn const_false() -> bool {
	false
}
