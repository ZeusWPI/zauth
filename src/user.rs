use diesel::{self, prelude::*};

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

use self::schema::user;
use self::schema::user::dsl::user as all_users;

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
		username: String,
		password: String,
		admin: bool,
		conn: &SqliteConnection,
	) -> bool
	{
		let u = User {
			id: None,
			username,
			password,
			admin,
		};
		diesel::insert_into(user::table)
			.values(&u)
			.execute(conn)
			.is_ok()
	}

	pub fn delete(self, conn: &SqliteConnection) -> bool {
		diesel::delete(all_users.find(self.id))
			.execute(conn)
			.is_ok()
	}
}
