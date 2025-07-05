use chrono::{NaiveDateTime, Utc};
use diesel::{query_dsl::methods::FilterDsl, ExpressionMethods, RunQueryDsl};
use validator::Validate;
use webauthn_rs::prelude::{CredentialID, Passkey};

use crate::{
	errors::{self, InternalError, Result, ZauthError},
	DbConn,
};

use self::schema::passkeys;

pub mod schema {
	table! {
		use diesel::sql_types::*;

		passkeys {
			id -> Integer,
			user_id -> Integer,
			name -> VarChar,
			cred -> VarChar,
			cred_id -> VarChar,
			last_used -> Timestamp,
			created_at -> Timestamp,
		}

	}
}

#[derive(
	Queryable, Selectable, PartialEq, Debug, Clone, Serialize, AsChangeset,
)]
#[diesel(table_name = passkeys)]
pub struct PassKey {
	pub id: i32,
	pub user_id: i32,
	pub name: String,
	#[serde(skip)]
	cred: String,
	#[serde(skip)]
	cred_id: String,
	pub last_used: NaiveDateTime,
	pub created_at: NaiveDateTime,
}

#[derive(Clone, Validate)]
pub struct NewPassKey {
	pub user_id: i32,
	#[validate(length(min = 1, max = 254))]
	pub name: String,
	pub cred: Passkey,
}

#[derive(Clone, Insertable)]
#[diesel(table_name = passkeys)]
struct NewPassKeySerialized {
	user_id: i32,
	name: String,
	cred: String,
	cred_id: String,
	last_used: NaiveDateTime,
}

impl PassKey {
	pub async fn create(
		passkey: NewPassKey,
		db: &DbConn,
	) -> errors::Result<PassKey> {
		passkey.validate()?;
		let serialized = NewPassKeySerialized {
			user_id: passkey.user_id,
			name: passkey.name,
			cred: serde_json::to_string(&passkey.cred)
				.map_err(InternalError::from)?,
			cred_id: serde_json::to_string(&passkey.cred.cred_id())
				.map_err(InternalError::from)?,
			last_used: Utc::now().naive_utc(),
		};

		db.run(move |conn| {
			diesel::insert_into(passkeys::table)
				.values(&serialized)
				.get_result::<PassKey>(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub async fn find(id: i32, db: &DbConn) -> errors::Result<Self> {
		db.run(move |conn| {
			diesel::QueryDsl::find(passkeys::table, id).first(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub async fn find_by_cred_id(
		cred_id: &CredentialID,
		db: &DbConn,
	) -> Result<Self> {
		let cred_id =
			serde_json::to_string(cred_id).map_err(InternalError::from)?;
		db.run(move |conn| {
			passkeys::table
				.filter(passkeys::cred_id.eq(cred_id))
				.first(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub fn credential(&self) -> Result<Passkey> {
		Ok(serde_json::from_str::<Passkey>(&self.cred)
			.map_err(InternalError::from)?)
	}

	pub fn set_credential(&mut self, cred: Passkey) -> Result<()> {
		self.cred =
			serde_json::to_string(&cred).map_err(InternalError::from)?;
		Ok(())
	}

	pub async fn find_credentials(
		user_id: i32,
		db: &DbConn,
	) -> errors::Result<Vec<Passkey>> {
		let keys = PassKey::find_by_user_id(user_id, db);
		Ok(keys
			.await?
			.iter()
			.filter_map(|key| key.credential().ok())
			.collect())
	}

	pub async fn find_by_user_id(
		user_id: i32,
		db: &DbConn,
	) -> errors::Result<Vec<PassKey>> {
		db.run(move |conn| {
			passkeys::table
				.filter(passkeys::user_id.eq(user_id))
				.get_results::<PassKey>(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub fn set_last_used(&mut self) {
		self.last_used = Utc::now().naive_utc();
	}

	pub async fn update(self, db: &DbConn) -> Result<Self> {
		let id = self.id;
		db.run(move |conn| {
			diesel::update(diesel::QueryDsl::find(passkeys::table, id))
				.set(self)
				.get_result(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub async fn delete(self, db: &DbConn) -> Result<()> {
		db.run(move |conn| {
			diesel::delete(passkeys::table.filter(passkeys::id.eq(self.id)))
				.execute(conn)
		})
		.await
		.map_err(ZauthError::from)?;
		Ok(())
	}
}
