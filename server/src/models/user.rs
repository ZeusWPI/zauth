use diesel::dsl::max;
use diesel::{self, prelude::*, Insertable, Queryable};

use self::schema::user;
use self::schema::user::dsl::user as all_users;

use bcrypt::{hash, verify, DEFAULT_COST};

#[table_name = "user"]
#[derive(Serialize, Queryable, Insertable, Debug, Clone)]
pub struct User {
	pub id:       Option<i32>,
	pub username: String,
	pub password: String,
	pub admin:    bool,
}

impl User {
	pub fn all(conn: &SqliteConnection) -> Vec<User> {
		all_users.order(user::id.desc()).load::<User>(conn).unwrap()
	}

	pub fn create(
		username: &str,
		password: &str,
		admin: bool,
		conn: &SqliteConnection,
	) -> Option<User>
	{
		let u = User {
			id: None,
			username: String::from(username),
			password: hash(password, DEFAULT_COST).unwrap(),
			admin,
		};
		conn.transaction(|| {
			diesel::insert_into(user::table).values(&u).execute(conn)?;

			all_users.order(user::id.desc()).first(conn)
		})
		.ok()
	}

	pub fn delete(self, conn: &SqliteConnection) -> bool {
		diesel::delete(all_users.find(self.id))
			.execute(conn)
			.is_ok()
	}

	pub fn find(id: i32, conn: &SqliteConnection) -> Option<User> {
		all_users.find(id).first(conn).ok()
	}

	pub fn find_and_authenticate(
		username: &String,
		password: &String,
		conn: &SqliteConnection,
	) -> Option<User>
	{
		all_users
			.filter(user::username.eq(username))
			.first(conn)
			.ok()
			.and_then(|user: User| {
				if verify(password, &user.password).unwrap_or(false) {
					Some(user)
				} else {
					None
				}
			})
	}
}

mod schema {
	table! {
		user {
			id -> Nullable<Integer>,
			username -> Text,
			password -> Text,
			admin -> Bool,
		}
	}
}
