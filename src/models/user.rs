use diesel::{self, prelude::*};

use self::schema::user;
use self::schema::user::dsl::user as users;

use bcrypt::*;

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

#[derive(Serialize, Queryable, Debug, Clone)]
pub struct User {
	pub id: i32,
	pub username: String,
	#[serde(skip)] // Let's not send our users their hashed password, shall we?
	pub hashed_password: String,
	pub admin: bool,
}

#[derive(FromForm, Debug, Clone)]
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

impl User {
	pub fn all(conn: &SqliteConnection) -> Vec<User> {
		users.order(user::id.desc()).load::<User>(conn).unwrap()
	}

	pub fn create(user: NewUser, conn: &SqliteConnection) -> Option<User> {
		let user = NewUserHashed {
			username:        user.username,
			hashed_password: hash(user.password, DEFAULT_COST).ok()?,
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

	pub fn find(id: i32, conn: &SqliteConnection) -> Option<User> {
		users.find(id).first(conn).ok()
	}

	pub fn find_and_authenticate(
		username: &str,
		password: &str,
		conn: &SqliteConnection,
	) -> Option<User>
	{
		users
			.filter(user::username.eq(username))
			.first(conn)
			.ok()
			.and_then(|user: User| {
				if verify(password, &user.hashed_password).unwrap_or(false) {
					Some(user)
				} else {
					None
				}
			})
	}
}
