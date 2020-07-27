use diesel::{self, prelude::*};

use self::schema::user;
use self::schema::user::dsl::user as users;
use crate::ConcreteConnection;

use pwhash::bcrypt::{self, BcryptSetup};

const DEFAULT_COST: u32 = 11;
const BCRYPT_SETUP: BcryptSetup = BcryptSetup {
	salt:    None,
	variant: None,
	cost:    Some(DEFAULT_COST),
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
	pub id:              i32,
	pub username:        String,
	#[serde(skip)] // Let's not send our users their hashed password, shall we?
	pub hashed_password: String,
	pub admin:           bool,
}

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct NewUser {
	pub username: String,
	pub password: String,
}

#[table_name = "user"]
#[derive(Serialize, Insertable, Debug, Clone)]
struct NewUserHashed {
	username:        String,
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
	pub fn all(conn: &ConcreteConnection) -> Vec<User> {
		users.load::<User>(conn).unwrap()
	}

	pub fn create(user: NewUser, conn: &ConcreteConnection) -> Option<User> {
		let user = NewUserHashed {
			username:        user.username,
			hashed_password: hash(&user.password).ok()?,
		};
		conn.transaction(|| {
			// Create a new user
			diesel::insert_into(user::table)
				.values(&user)
				.execute(conn)?;
			// Fetch the last created user
			users.order(user::id.desc()).first(conn)
		})
		.ok()
	}

	pub fn change_with(&mut self, change: UserChange) -> Option<()> {
		if let Some(username) = change.username {
			self.username = username;
		}
		if let Some(password) = change.password {
			self.hashed_password = hash(&password).ok()?;
		}
		Some(())
	}

	pub fn update(self, conn: &ConcreteConnection) -> Option<Self> {
		let id = self.id;
		Some(
			conn.transaction(|| {
				// Create a new user
				diesel::update(users.find(id))
					.set(self)
					.execute(conn)
					.map(|_| ())?;
				// Fetch the updated record
				users.find(id).first(conn)
			})
			.unwrap(),
		)
	}

	pub fn find(id: i32, conn: &ConcreteConnection) -> Option<User> {
		users.find(id).first(conn).ok()
	}

	pub fn last(conn: &ConcreteConnection) -> Option<User> {
		users.order(user::id.desc()).first(conn).ok()
	}

	pub fn find_and_authenticate(
		username: &str,
		password: &str,
		conn: &ConcreteConnection,
	) -> Option<User>
	{
		users
			.filter(user::username.eq(username))
			.first(conn)
			.ok()
			.and_then(|user: User| {
				if verify(password, &user.hashed_password) {
					Some(user)
				} else {
					None
				}
			})
	}
}

fn hash(password: &str) -> Result<String, pwhash::error::Error> {
	bcrypt::hash_with(BCRYPT_SETUP, password)
}

fn verify(password: &str, hash: &str) -> bool {
	bcrypt::verify(password, &hash)
}
