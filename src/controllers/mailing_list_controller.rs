use lettre::message::Mailbox;
use rocket::response::{Redirect, Responder};
use std::fmt::Debug;

use crate::ephemeral::from_api::Api;
use crate::ephemeral::session::AdminSession;
use crate::errors::Result;
use crate::mailer::Mailer;
use crate::models::mail::*;
use crate::models::user::User;
use crate::views::accepter::Accepter;
use crate::DbConn;
use askama::Template;
use rocket::serde::json::Json;
use rocket::State;

/// Show an overview of all mails, sorted by send date
#[get("/mails")]
pub async fn list_mails<'r>(
	session: AdminSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let mails = Mail::all(&db).await?;

	Ok(Accepter {
		html: template! {
			"maillist/index.html";
			current_user: User = session.admin,
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
	mailer: &'r State<Mailer>,
) -> Result<impl Responder<'r, 'static>> {
	let mail = new_mail.into_inner().save(&db).await?;

	let subscribed_users = User::find_subscribed(&db).await?;
	let bcc = subscribed_users
		.iter()
		.map(|u| Mailbox::try_from(u))
		.collect::<Result<Vec<Mailbox>>>()?;

	mailer.try_create_with_bcc(bcc, mail.subject.clone(), mail.body.clone())?;

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
	session: AdminSession,
	db: DbConn,
	id: i32,
) -> Result<impl Responder<'r, 'static>> {
	let mail = Mail::get_by_id(id, &db).await?;

	Ok(Accepter {
		html: template! {
			"maillist/show_mail.html";
			current_user: User = session.admin,
			mail: Mail = mail.clone(),
		},
		json: Json(mail),
	})
}
