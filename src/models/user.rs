use super::schema::{roles, users};
use crate::DbConn;
use crate::errors::{self, InternalError, LoginError, ZauthError};
use diesel::{self, prelude::*};
use diesel_derive_enum::DbEnum;
use std::fmt;
use std::sync::LazyLock;

use crate::Config;
use crate::util::random_token;
use chrono::{NaiveDateTime, Utc};
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use lettre::message::Mailbox;
use pwhash::bcrypt::{self, BcryptSetup};
use regex::Regex;
use rocket::{FromFormField, serde::Serialize};
use std::convert::TryFrom;
use validator::{Validate, ValidationError, ValidationErrors};

use super::role::{Role, UserRole};

#[derive(
	DbEnum,
	Debug,
	Deserialize,
	FromFormField,
	Serialize,
	Clone,
	PartialEq,
	QueryId,
)]
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

#[derive(
	Validate,
	Serialize,
	AsChangeset,
	Selectable,
	Queryable,
	Debug,
	Clone,
	PartialEq,
	Identifiable,
)]
#[diesel(table_name = users)]
#[diesel(treat_none_as_null = true)]
#[serde(crate = "rocket::serde")]
pub struct User {
	pub id: i32,
	#[validate(length(min = 1, max = 254))]
	pub username: String,
	#[serde(skip)] // Let's not send our users their hashed password, shall we?
	pub hashed_password: String,
	#[serde(skip)]
	pub password_reset_token: Option<String>,
	#[serde(skip)]
	pub password_reset_expiry: Option<NaiveDateTime>,
	pub admin: bool,
	#[validate(length(min = 3, max = 254))]
	pub full_name: String,
	#[validate(email)]
	#[serde(skip)]
	// Don't send backing email address of users, applications could
	// accidentally use this
	pub email: String,
	#[validate(custom(function = "validate_ssh_key_list"))]
	pub ssh_key: Option<String>,
	#[serde(skip)]
	pub state: UserState,
	pub last_login: NaiveDateTime,
	pub created_at: NaiveDateTime,
	#[serde(skip)]
	pub pending_email: Option<String>,
	#[serde(skip)]
	pub pending_email_token: Option<String>,
	#[serde(skip)]
	pub pending_email_expiry: Option<NaiveDateTime>,
	pub subscribed_to_mailing_list: bool,
	#[serde(skip)]
	pub unsubscribe_token: String,
}

static NEW_USER_REGEX: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"^[a-z][-a-z0-9_]{2,31}$").unwrap());

#[derive(Validate, FromForm, Deserialize, Debug, Clone)]
pub struct NewUser {
	#[validate(regex(
		path = *NEW_USER_REGEX,
		message = r"Username didn't match regex /^[a-z][-a-z0-9_]{2,31}$/ (don't use uppercase letters).",
	))]
	pub username: String,
	#[validate(length(
		min = 8,
		message = "Password too short, must be at least 8 characters."
	))]
	pub password: String,
	#[validate(length(min = 3, max = 254))]
	pub full_name: String,
	#[validate(email)]
	pub email: String,
	#[validate(custom(function = "validate_ssh_key_list"))]
	pub ssh_key: Option<String>,
	#[validate(custom(function = "validate_not_a_robot"))]
	#[serde(default = "const_false")]
	pub not_a_robot: bool,
}

#[derive(Serialize, Insertable, Debug, Clone)]
#[diesel(table_name = users)]
struct PendingUserHashed {
	username: String,
	hashed_password: String,
	full_name: String,
	state: UserState,
	last_login: NaiveDateTime,
	email: String,
	pending_email: String,
	pending_email_token: String,
	pending_email_expiry: NaiveDateTime,
}

#[derive(Serialize, Insertable, Debug, Clone)]
#[diesel(table_name = users)]
struct NewUserHashed {
	username: String,
	hashed_password: String,
	full_name: String,
	state: UserState,
	last_login: NaiveDateTime,
	email: String,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct UserChange {
	pub username: Option<String>,
	pub password: Option<String>,
	pub full_name: Option<String>,
	pub email: Option<String>,
	pub ssh_key: Option<String>,
	pub subscribed_to_mailing_list: bool,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct ChangeAdmin {
	pub admin: bool,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct ChangeStatus {
	pub state: UserState,
}

#[derive(Validate, FromForm, Deserialize, Debug, Clone)]
pub struct ChangePassword {
	#[validate(length(
		min = 8,
		message = "Password too short, must be at least 8 characters."
	))]
	pub password: String,
}

impl User {
	pub async fn all(db: &DbConn) -> errors::Result<Vec<User>> {
		let all_users =
			db.run(move |conn| users::table.load::<User>(conn)).await?;
		Ok(all_users)
	}

	pub fn is_active(&self) -> bool {
		matches!(self.state, UserState::Active)
	}

	pub async fn find_by_username<'r>(
		username: String,
		db: &DbConn,
	) -> errors::Result<User> {
		db.run(move |conn| {
			users::table
				.filter(users::username.eq(username))
				.first(conn)
				.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn find_by_email(
		email: String,
		db: &DbConn,
	) -> errors::Result<User> {
		let query = users::table.filter(users::email.eq(email));
		db.run(move |conn| query.first(conn).map_err(ZauthError::from))
			.await
	}

	/// Find all active users that are subscribed to the mailing list
	pub async fn find_subscribed(db: &DbConn) -> errors::Result<Vec<Self>> {
		db.run(move |conn| {
			users::table
				.filter(users::subscribed_to_mailing_list.eq(true))
				.filter(users::state.eq(UserState::Active))
				.load::<Self>(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	/// Find a user given their unique unsubscribe token
	pub async fn find_by_unsubscribe_token<'r>(
		token: String,
		db: &DbConn,
	) -> errors::Result<Option<User>> {
		let token = token.to_owned();
		let result = db
			.run(move |conn| {
				users::table
					.filter(users::unsubscribe_token.eq(token))
					.first::<Self>(conn)
			})
			.await
			.map_err(ZauthError::from);

		match result {
			Ok(user) => Ok(Some(user)),
			Err(ZauthError::NotFound(_)) => Ok(None),
			Err(e) => Err(e),
		}
	}

	pub async fn delete(self, db: &DbConn) -> errors::Result<()> {
		db.run(move |conn| {
			diesel::delete(users::table.find(self.id))
				.execute(conn)
				.map_err(ZauthError::from)
		})
		.await?;
		Ok(())
	}

	pub async fn find_by_password_token<'r>(
		token: String,
		db: &DbConn,
	) -> errors::Result<Option<User>> {
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

	fn email_token_valid(&self) -> bool {
		if let Some(expiry) = self.pending_email_expiry {
			return Utc::now().naive_utc() < expiry;
		}
		false
	}

	pub async fn find_by_email_token<'r>(
		token: String,
		db: &DbConn,
	) -> errors::Result<Option<User>> {
		let token = token.to_owned();
		let result = db
			.run(move |conn| {
				users::table
					.filter(users::pending_email_token.eq(token))
					.first::<Self>(conn)
			})
			.await
			.map_err(ZauthError::from);
		match result {
			Ok(user) if user.email_token_valid() => Ok(Some(user)),
			Ok(_) => Ok(None),
			Err(ZauthError::NotFound(_)) => Ok(None),
			Err(err) => Err(err),
		}
	}

	pub async fn find_by_pending(db: &DbConn) -> errors::Result<Vec<User>> {
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
	) -> errors::Result<User> {
		user.validate()?;
		let user = NewUserHashed {
			username: user.username,
			hashed_password: hash(&user.password, bcrypt_cost)?,
			full_name: user.full_name,
			email: user.email,
			state: UserState::Active,
			last_login: Utc::now().naive_utc(),
		};
		db.run(move |conn| {
			conn.transaction(|conn| {
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
		.map_err(db_error_to_client_error)
	}

	pub async fn create_pending(
		user: NewUser,
		conf: &Config,
		db: &DbConn,
	) -> errors::Result<User> {
		user.validate()?;
		if Self::pending_count(db).await? >= conf.maximum_pending_users {
			let mut err = ValidationErrors::new();
			err.add(
				"__all__",
				ValidationError::new(
					"Because of an unusual amount of registrations, we have \
					 temporarily disabled registrations. Please come back \
					 later or contact an admin to request an account.",
				),
			);
			return Err(ZauthError::from(err));
		};
		let user = PendingUserHashed {
			username: user.username,
			hashed_password: hash(&user.password, conf.bcrypt_cost)?,
			full_name: user.full_name,
			email: user.email.clone(),
			pending_email: user.email,
			pending_email_token: random_token(conf.secure_token_length),
			pending_email_expiry: Utc::now().naive_utc()
				+ conf.email_confirmation_token_duration(),
			state: UserState::PendingMailConfirmation,
			last_login: Utc::now().naive_utc(),
		};
		db.run(move |conn| {
			conn.transaction(|conn| {
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
		.map_err(db_error_to_client_error)
	}

	pub async fn approve(mut self, db: &DbConn) -> errors::Result<User> {
		if self.state != UserState::PendingApproval {
			return Err(ZauthError::Unprocessable(String::from(
				"user is not in the pending approval state",
			)));
		}
		self.state = UserState::Active;
		self.update(db).await
	}

	pub async fn confirm_email(mut self, db: &DbConn) -> errors::Result<User> {
		if self.state == UserState::PendingMailConfirmation {
			self.state = UserState::PendingApproval;
		}
		self.email = self
			.pending_email
			.as_ref()
			.ok_or(ZauthError::Unprocessable(String::from(
				"valid confirmation token, but no pending email",
			)))?
			.to_string();
		self.pending_email_token = None;
		self.pending_email_expiry = None;
		self.update(db).await
	}

	pub fn change_with(&mut self, change: UserChange) -> errors::Result<()> {
		if let Some(email) = change.email {
			self.email = email;
		}
		if let Some(ssh_key) = change.ssh_key {
			self.ssh_key = Some(ssh_key);
		}
		self.subscribed_to_mailing_list = change.subscribed_to_mailing_list;
		self.validate()?;
		Ok(())
	}

	pub async fn update(self, db: &DbConn) -> errors::Result<Self> {
		let id = self.id;
		db.run(move |conn| {
			conn.transaction(|conn| {
				// Create a new user
				diesel::update(users::table.find(id))
					.set(self)
					.execute(conn)?;

				// Fetch the updated record
				users::table.find(id).first(conn)
			})
		})
		.await
		.map_err(db_error_to_client_error)
	}

	pub async fn change_password(
		mut self,
		change: ChangePassword,
		conf: &Config,
		db: &DbConn,
	) -> errors::Result<Self> {
		change.validate()?;
		self.hashed_password = hash(&change.password, conf.bcrypt_cost)?;
		self.password_reset_token = None;
		self.password_reset_expiry = None;
		self.update(db).await
	}

	pub async fn reload(self, db: &DbConn) -> errors::Result<User> {
		Self::find(self.id, db).await
	}

	pub async fn update_last_login(
		mut self,
		db: &DbConn,
	) -> errors::Result<Self> {
		self.last_login = Utc::now().naive_utc();
		self.update(db).await
	}

	pub async fn find(id: i32, db: &DbConn) -> errors::Result<User> {
		db.run(move |conn| {
			users::table.find(id).first(conn).map_err(ZauthError::from)
		})
		.await
	}

	pub async fn pending_count(db: &DbConn) -> errors::Result<usize> {
		let count: i64 = db
			.run(move |conn| {
				users::table
					.filter(users::state.eq(UserState::PendingApproval))
					.or_filter(
						users::state.eq(UserState::PendingMailConfirmation),
					)
					.count()
					.first(conn)
					.map_err(ZauthError::from)
			})
			.await?;
		Ok(count as usize)
	}

	pub async fn last(db: &DbConn) -> errors::Result<User> {
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
	) -> errors::Result<User> {
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
			Err(e) => Err(e),
		}
	}

	pub async fn roles(self, db: &DbConn) -> Result<Vec<Role>, ZauthError> {
		db.run(move |conn| {
			UserRole::belonging_to(&self)
				.inner_join(roles::table)
				.select(Role::as_select())
				.load(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub async fn roles_for_client(
		self,
		client_id: i32,
		db: &DbConn,
	) -> Result<Vec<Role>, ZauthError> {
		db.run(move |conn| {
			UserRole::belonging_to(&self)
				.inner_join(roles::table)
				.filter(
					roles::client_id
						.eq(client_id)
						.or(roles::client_id.is_null()),
				)
				.select(Role::as_select())
				.load(conn)
		})
		.await
		.map_err(ZauthError::from)
	}
}

fn hash(
	password: &str,
	bcrypt_cost: u32,
) -> crate::errors::InternalResult<String> {
	let b: BcryptSetup = BcryptSetup {
		salt: None,
		variant: None,
		cost: Some(bcrypt_cost),
	};
	let hashed = bcrypt::hash_with(b, password)?;
	Ok(hashed)
}

fn verify(password: &str, hash: &str) -> bool {
	bcrypt::verify(password, hash)
}

impl TryFrom<&User> for Mailbox {
	type Error = ZauthError;

	fn try_from(value: &User) -> errors::Result<Mailbox> {
		Ok(Mailbox::new(
			Some(value.username.to_string()),
			value.email.clone().parse().map_err(InternalError::from)?,
		))
	}
}

fn validate_ssh_key_list(ssh_keys: &String) -> Result<(), ValidationError> {
	lazy_static! {
		static ref SSH_KEY_REGEX: Regex = Regex::new(
			r"(?x)^
				(
					ssh-(rsa|dss|ecdsa|ed25519)|
					ecdsa-sha2-nistp(256|384|521)|
					sk-(
						ecdsa-sha2-nistp256@openssh.com|
						ssh-ed25519@openssh.com
					)
				)
				\s[a-zA-Z0-9+/]{1,750}={0,3}( \S+)?"
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
			 interface. Please interface with an AIdmin.",
		));
	}
	Ok(())
}

// These constraints are not explicitly named in the database migration scripts:
// {table}_{column}_key is the default name given to UNIQUE column constraints
// in postgresql.
const USERNAME_UNIQUENESS_CONSTRAINT_NAME: &str = "users_username_key";
const EMAIL_UNIQUENESS_CONSTRAINT_NAME: &str = "users_email_key";

/// Map an database error to an error in the user domain.
///
/// Normally, a database error would result in an InternalError (5xx) - server
/// side error. In some cases however, it is clear that these errors are the
/// direct consequence of user actions, such as when an user requests to
/// register an username that is already taken. (UNIQUE constraint violation).
/// In these cases we would like to 'lift' the internal error into the client
/// error (4xx) realm. This pattern is desirable because it allows us to do
/// atomic constraint checking.
///
/// In summary: `run_my_query().map_err(db_error_to_client_error)` basically
/// means "no its your fault".
fn db_error_to_client_error(error: DieselError) -> ZauthError {
	match error {
		DieselError::DatabaseError(
			DatabaseErrorKind::UniqueViolation,
			info,
		) if info.constraint_name()
			== Some(USERNAME_UNIQUENESS_CONSTRAINT_NAME) =>
		{
			let mut err = ValidationErrors::new();
			err.add(
				"username",
				ValidationError::new("Username already taken."),
			);
			ZauthError::from(err)
		},
		DieselError::DatabaseError(
			DatabaseErrorKind::UniqueViolation,
			info,
		) if info.constraint_name()
			== Some(EMAIL_UNIQUENESS_CONSTRAINT_NAME) =>
		{
			let mut err = ValidationErrors::new();
			err.add("email", ValidationError::new("Email already taken."));
			ZauthError::from(err)
		},
		other => ZauthError::from(other),
	}
}

/// used as a default for not_a_robot field
fn const_false() -> bool {
	false
}
