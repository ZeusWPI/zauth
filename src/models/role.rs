use crate::{
	DbConn,
	errors::{Result, ZauthError},
};
use diesel::{self, prelude::*};
use validator::Validate;

use crate::models::schema::{clients_roles, roles, users, users_roles};
use crate::models::{client::Client, user::User};

#[derive(
	Deserialize,
	Serialize,
	Queryable,
	Debug,
	Clone,
	Identifiable,
	PartialEq,
	Selectable,
)]
pub struct Role {
	pub id: i32,
	pub name: String,
	pub description: String,
	pub client_id: Option<i32>,
}

#[derive(Validate, FromForm, Debug, Insertable, Deserialize)]
#[diesel(table_name = roles)]
pub struct NewRole {
	#[validate(length(min = 1, max = 30))]
	pub name: String,
	#[validate(length(min = 1, max = 100))]
	pub description: String,
	pub client_id: Option<i32>,
}

#[derive(
	Identifiable, Selectable, Queryable, Associations, Debug, Insertable,
)]
#[diesel(belongs_to(Role))]
#[diesel(belongs_to(User))]
#[diesel(table_name = users_roles)]
#[diesel(primary_key(role_id, user_id))]
pub struct UserRole {
	pub role_id: i32,
	pub user_id: i32,
}

#[derive(
	Identifiable, Selectable, Queryable, Associations, Debug, Insertable,
)]
#[diesel(belongs_to(Role))]
#[diesel(belongs_to(Client))]
#[diesel(table_name = clients_roles)]
#[diesel(primary_key(role_id, client_id))]
pub struct ClientRole {
	pub role_id: i32,
	pub client_id: i32,
}

impl Role {
	pub async fn create(role: NewRole, db: &DbConn) -> Result<Role> {
		role.validate()?;

		db.run(move |conn| {
			diesel::insert_into(roles::table)
				.values(&role)
				.get_result::<Role>(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub async fn add_user(&self, user_id: i32, db: &DbConn) -> Result<bool> {
		let id = self.id;
		let user_role = db
			.run(move |conn| {
				users_roles::table
					.filter(users_roles::user_id.eq(user_id))
					.filter(users_roles::role_id.eq(id))
					.first::<UserRole>(conn)
					.optional()
			})
			.await
			.map_err(ZauthError::from)?;

		if user_role.is_none() {
			// UserRole not already exists
			let user_role = UserRole {
				role_id: self.id,
				user_id,
			};
			db.run(move |conn| {
				diesel::insert_into(users_roles::table)
					.values(&user_role)
					.execute(conn)
			})
			.await
			.map_err(ZauthError::from)?;
			Ok(true)
		} else {
			Ok(false)
		}
	}

	pub async fn add_client(
		&self,
		client_id: i32,
		db: &DbConn,
	) -> Result<bool> {
		let id = self.id;
		let client_role = db
			.run(move |conn| {
				clients_roles::table
					.filter(clients_roles::client_id.eq(client_id))
					.filter(clients_roles::role_id.eq(id))
					.first::<ClientRole>(conn)
					.optional()
			})
			.await
			.map_err(ZauthError::from)?;

		if client_role.is_none() {
			// UserRole not already exists
			let client_role = ClientRole {
				role_id: self.id,
				client_id,
			};
			db.run(move |conn| {
				diesel::insert_into(clients_roles::table)
					.values(&client_role)
					.execute(conn)
			})
			.await
			.map_err(ZauthError::from)?;
			Ok(true)
		} else {
			Ok(false)
		}
	}

	pub async fn remove_user(self, user_id: i32, db: &DbConn) -> Result<bool> {
		let count = db
			.run(move |conn| {
				diesel::delete(
					users_roles::table
						.filter(users_roles::user_id.eq(user_id))
						.filter(users_roles::role_id.eq(self.id)),
				)
				.execute(conn)
			})
			.await
			.map_err(ZauthError::from)?;
		Ok(count > 0)
	}

	pub async fn remove_client(
		self,
		client_id: i32,
		db: &DbConn,
	) -> Result<bool> {
		let count = db
			.run(move |conn| {
				diesel::delete(
					clients_roles::table
						.filter(clients_roles::client_id.eq(client_id))
						.filter(clients_roles::role_id.eq(self.id)),
				)
				.execute(conn)
			})
			.await
			.map_err(ZauthError::from)?;
		Ok(count > 0)
	}

	pub async fn users(self, db: &DbConn) -> Result<Vec<User>> {
		db.run(move |conn| {
			UserRole::belonging_to(&self)
				.inner_join(users::table)
				.select(User::as_select())
				.load(conn)
		})
		.await
		.map_err(ZauthError::from)
	}

	pub async fn find(id: i32, db: &DbConn) -> Result<Self> {
		db.run(move |conn| diesel::QueryDsl::find(roles::table, id).first(conn))
			.await
			.map_err(ZauthError::from)
	}

	pub async fn all(db: &DbConn) -> Result<Vec<Role>> {
		let all_roles =
			db.run(move |conn| roles::table.load::<Role>(conn)).await?;
		Ok(all_roles)
	}

	pub async fn delete(self, db: &DbConn) -> Result<()> {
		db.run(move |conn| {
			diesel::delete(roles::table.filter(roles::id.eq(self.id)))
				.execute(conn)
		})
		.await
		.map_err(ZauthError::from)?;
		Ok(())
	}
}
