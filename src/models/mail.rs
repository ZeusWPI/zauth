use std::cmp::Reverse;

use crate::DbConn;
use crate::errors::{self, ZauthError};
use chrono::NaiveDateTime;
use diesel::{self, prelude::*};
use diesel_derive_enum::DbEnum;
use markdown::{Options, to_html_with_options};

use diesel::result::Error as DieselError;
use rocket::serde::Serialize;
use validator::Validate;

use super::schema::mails;

#[derive(DbEnum, Debug, Deserialize, FromFormField, Serialize, Copy, Clone)]
#[db_enum(existing_type_path = "crate::models::schema::sql_types::ContentType")]
pub enum ContentType {
	#[db_enum(rename = "text/plain")]
	#[serde(rename = "text/plain")]
	#[field(value = "text/plain")]
	Plain,
	#[db_enum(rename = "text/markdown")]
	#[serde(rename = "text/markdown")]
	#[field(value = "text/markdown")]
	Markdown,
}

#[derive(Clone, Debug, Queryable, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Mail {
	pub id: i32,
	pub sent_on: NaiveDateTime,
	pub subject: String,
	pub body: String,
	pub author: String,
	pub content_type: ContentType,
}

#[derive(
	Clone, Debug, Deserialize, Serialize, FromForm, Insertable, Validate,
)]
#[diesel(table_name = mails)]
pub struct NewMail {
	pub author: String,
	#[validate(length(min = 3, max = 255))]
	pub subject: String,
	#[validate(length(min = 3, max = 10_000))]
	pub body: String,
	pub content_type: ContentType,
}

impl NewMail {
	/// Save the [`NewMail`] to the database and return the newly stored
	/// [`Mail`] object
	pub async fn save(self, db: &DbConn) -> errors::Result<Mail> {
		self.validate()?;

		db.run(move |conn| {
			conn.transaction::<_, DieselError, _>(|conn| {
				// Insert the new mail
				diesel::insert_into(mails::table)
					.values(&self)
					.execute(conn)?;

				// Return the newly inserted mail
				let mail = mails::table.order(mails::id.desc()).first(conn)?;
				Ok(mail)
			})
		})
		.await
		.map_err(ZauthError::from)
	}
}

impl Mail {
	/// Get a list of all [`Mail`]s, sorted by the `sent_on` date
	pub async fn all(db: &DbConn) -> errors::Result<Vec<Self>> {
		let mut all_mails =
			db.run(move |conn| mails::table.load::<Self>(conn)).await?;

		all_mails.sort_by_key(|m| Reverse(m.sent_on));

		Ok(all_mails)
	}

	/// Get a mail given its id
	pub async fn get_by_id(id: i32, db: &DbConn) -> errors::Result<Self> {
		db.run(move |conn| {
			mails::table.find(id).first(conn).map_err(ZauthError::from)
		})
		.await
	}

	pub fn render_body(&self) -> errors::Result<String> {
		match self.content_type {
			ContentType::Plain => Ok(self.body.clone()),
			ContentType::Markdown => to_html_with_options(
				&self.body,
				&Options::gfm(),
			)
			.map_err(|_| {
				ZauthError::Unprocessable("could not parse markdown".into())
			}),
		}
	}
}
