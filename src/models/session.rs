use chrono::{Duration, NaiveDateTime, Utc};
use diesel::{self, prelude::*};

use crate::DbConn;

use self::schema::sessions;
use crate::config::Config;
use crate::errors::{Result, ZauthError};
use crate::models::client::Client;
use crate::models::user::User;
use crate::util::random_token;

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
			scope -> Nullable<Text>,
		}
	}
}

#[derive(Serialize, AsChangeset, Queryable, Associations, Debug, Clone)]
#[diesel(belongs_to(User))]
#[diesel(belongs_to(Client))]
#[diesel(table_name = sessions)]
pub struct Session {
	pub id:         i32,
	pub key:        Option<String>,
	pub user_id:    i32,
	pub client_id:  Option<i32>,
	pub created_at: NaiveDateTime,
	pub expires_at: NaiveDateTime,
	pub valid:      bool,
	pub scope:      Option<String>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = sessions)]
pub struct NewSession {
	pub key:        Option<String>,
	pub user_id:    i32,
	pub client_id:  Option<i32>,
	pub created_at: NaiveDateTime,
	pub expires_at: NaiveDateTime,
}

impl Session {
	pub async fn create(
		user: &User,
		session_duration: Duration,
		db: &DbConn,
	) -> Result<Session> {
		let created_at = Utc::now().naive_utc();
		let expires_at = created_at + session_duration;
		let session = NewSession {
			user_id: user.id,
			client_id: None,
			key: None,
			created_at,
			expires_at,
		};
		db.run(move |conn| {
			conn.transaction(|conn| {
				// Create a new session
				diesel::insert_into(sessions::table)
					.values(&session)
					.execute(conn)?;
				// Fetch the last created session
				sessions::table.order(sessions::id.desc()).first(conn)
			})
			.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn create_client_session(
		user: &User,
		client: &Client,
		conf: &Config,
		db: &DbConn,
	) -> Result<Session> {
		let created_at = Utc::now().naive_utc();
		let expires_at = created_at + conf.client_session_duration();
		let key = random_token(conf.secure_token_length);
		let session = NewSession {
			user_id: user.id,
			client_id: Some(client.id),
			key: Some(key),
			created_at,
			expires_at,
		};
		db.run(move |conn| {
			conn.transaction(|conn| {
				// Create a new session
				diesel::insert_into(sessions::table)
					.values(&session)
					.execute(conn)?;
				// Fetch the last created session
				sessions::table.order(sessions::id.desc()).first(conn)
			})
			.map_err(ZauthError::from)
		})
		.await
	}

	pub async fn update(self, db: &DbConn) -> Result<Self> {
		let id = self.id;
		db.run(move |conn| {
			conn.transaction(|conn| {
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
		let now = Utc::now().naive_utc();
		let session = db
			.run(move |conn| {
				sessions::table
					.filter(sessions::key.eq(Some(key)))
					.filter(sessions::valid.eq(true))
					.filter(sessions::expires_at.gt(now))
					.first(conn)
					.map_err(ZauthError::from)
			})
			.await?;
		Ok(session)
	}

	pub async fn find_by_id(id: i32, db: &DbConn) -> Result<Session> {
		let now = Utc::now().naive_utc();
		let session = db
			.run(move |conn| {
				sessions::table
					.filter(sessions::valid.eq(true))
					.filter(sessions::id.eq(id))
					.filter(sessions::expires_at.gt(now))
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

	pub async fn client(&self, db: &DbConn) -> Result<Option<Client>> {
		if let Some(client_id) = self.client_id {
			Ok(Some(Client::find(client_id, &db).await?))
		} else {
			Ok(None)
		}
	}

	pub async fn last(db: &DbConn) -> Result<Session> {
		Ok(db
			.run(move |conn| {
				sessions::table.order(sessions::id.desc()).first(conn)
			})
			.await?)
	}
}
