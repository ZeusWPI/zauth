use lettre::message::Mailbox;
use rocket::response::{Redirect, Responder};
use std::fmt::Debug;

use crate::DbConn;
use crate::config::Config;
use crate::controllers::users_controller::rocket_uri_macro_show_confirm_unsubscribe;
use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::{AdminSession, UserSession};
use crate::errors::{InternalError, Result};
use crate::mailer::Mailer;
use crate::models::mail::*;
use crate::models::user::User;
use crate::views::accepter::Accepter;
use askama::Template;
use rocket::State;
use rocket::serde::json::Json;

/// Show an overview of all mails, sorted by send date
#[get("/mails")]
pub async fn list_mails<'r>(
	session: UserSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mails = Mail::all(&db).await?;

	Ok(Accepter {
		html: template! {
			"maillist/index.html";
			current_user: User = session.user,
			mails: Vec<Mail> = mails.clone(),
		},
		json: Json(mails),
	})
}

/// Send a new mail and archive it
#[post("/mails", data = "<new_mail>")]
pub async fn send_mail<'r>(
	_session: AdminSession,
	new_mail: Api<NewMail>,
	db: DbConn,
	conf: &'r State<Config>,
	mailer: &'r State<Mailer>,
) -> Result<impl Responder<'r, 'static>> {
	let mail = new_mail.into_inner().save(&db).await?;

	let subscribed_users = User::find_subscribed(&db).await?;

	for user in &subscribed_users {
		let receiver = Mailbox::try_from(user)?;
		let token = user.unsubscribe_token.to_string();
		let unsubscribe_url =
			uri!(conf.base_url(), show_confirm_unsubscribe(token));

		let body = template!(
			"mails/mailinglist_mail.txt";
			body: String = mail.body.clone(),
			unsubscribe_url: String = unsubscribe_url.to_string(),
		)
		.render()
		.map_err(InternalError::from)?;

		mailer.create_for_mailinglist(receiver, mail.subject.clone(), body)?;
	}

	Ok(Accepter {
		html: Redirect::to(uri!(show_mail(mail.id))),
		json: Json(mail),
	})
}

/// Show the new_mail page
#[get("/mails/new")]
pub async fn show_create_mail_page<'r>(
	session: AdminSession,
) -> Result<impl Responder<'r, 'static>> {
	Ok(template! {
		"maillist/new_mail.html";
		current_user: User = session.admin,
	})
}

/// Show a specific mail
#[get("/mails/<id>")]
pub async fn show_mail<'r>(
	session: UserSession,
	db: DbConn,
	id: i32,
) -> Result<impl Responder<'r, 'static>> {
	let mail = Mail::get_by_id(id, &db).await?;

	Ok(Accepter {
		html: template! {
			"maillist/show_mail.html";
			current_user: User = session.user,
			mail: Mail = mail.clone(),
		},
		json: Json(mail),
	})
}
