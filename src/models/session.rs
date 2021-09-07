use diesel::{self, prelude::*};

use crate::DbConn;

use self::schema::sessions;
use crate::errors::{Result, ZauthError};
use crate::models::client::Client;
use crate::models::user::User;

use chrono::NaiveDateTime;

pub mod schema {
	table! {
		sessions {
			id -> Integer,
			key -> Nullable<VarChar>,
			user_id -> Integer,
			client_id -> Nullable<Integer>,
			created_at -> Timestamp,
			expires_at -> Timestamp,
			valid -> Bool,
			scopes -> Text,
		}
	}
}

#[derive(Serialize, AsChangeset, Queryable, Associations, Debug, Clone)]
#[belongs_to(User)]
#[belongs_to(Client)]
#[table_name = "sessions"]
pub struct Session {
	pub id:         i32,
	pub key:        Option<String>,
	pub user_id:    i32,
	pub client_id:  Option<i32>,
	pub created_at: NaiveDateTime,
	pub expires_at: NaiveDateTime,
	pub valid:      bool,
	pub scopes:     String,
}

#[derive(Insertable, Debug, Clone)]
#[table_name = "sessions"]
pub struct NewSession {
	pub key:       Option<String>,
	pub user_id:   i32,
	pub client_id: Option<i32>,
}

impl Session {
	pub async fn create(user: &User, db: &DbConn) -> Result<Session> {
		let session = NewSession {
			user_id:   user.id,
			client_id: None,
			key:       None,
		};
		db.run(move |conn| {
			conn.transaction(|| {
				// Create a new client
				diesel::insert_into(sessions::table)
					.values(&session)
					.execute(conn)?;
				// Fetch the last created client
				sessions::table.order(sessions::id.desc()).first(conn)
			})
			.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn update(self, db: &DbConn) -> Result<Self> {
		let id = self.id;
		db.run(move |conn| {
			conn.transaction(|| {
				// Update a session
				diesel::update(sessions::table.find(id))
					.set(self)
					.execute(conn)?;

				// Fetch the updated record
				sessions::table.find(id).first(conn)
			})
			.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn find_by_key(key: String, db: &DbConn) -> Result<Session> {
		let session = db
			.run(move |conn| {
				sessions::table
					.filter(sessions::key.eq(Some(key)))
					.filter(sessions::valid.eq(true))
					.first(conn)
					.map_err(ZauthError::from)
			})
			.await?;
		Ok(session)
	}

	pub async fn find_by_id(id: i32, db: &DbConn) -> Result<Session> {
		let session = db
			.run(move |conn| {
				sessions::table
					.filter(sessions::valid.eq(true))
					.filter(sessions::id.eq(id))
					.first(conn)
			})
			.await?;
		Ok(session)
	}

	pub async fn invalidate(mut self, db: &DbConn) -> Result<Session> {
		self.valid = false;
		self.update(db).await
	}

	pub async fn user(&self, db: &DbConn) -> Result<User> {
		User::find(self.user_id, &db).await
	}
}
