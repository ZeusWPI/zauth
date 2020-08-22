use crate::errors::{InternalError, Result, ZauthError};
use crate::models::user::User;

use lettre::{SendableEmail, SmtpTransport, Transport};
use lettre_email::Email;
use std::sync::mpsc;
use std::thread;

const MAILQUEUE_BOUND: usize = 32;

#[derive(Clone)]
pub struct Mailer {
	from:  String,
	queue: mpsc::SyncSender<SendableEmail>,
}

impl Mailer {
	pub fn create(
		&self,
		user: &User,
		subject: String,
		text: String,
	) -> Result<()>
	{
		let mail: SendableEmail = Email::builder()
			.to(user)
			.subject(subject)
			.from(self.from.clone())
			.text(text)
			.build()
			.map_err(InternalError::from)?
			.into();

		self.queue
			.try_send(mail)
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}
}

pub fn create_queue(from: String, mut transport: SmtpTransport) -> Mailer {
	let (sender, recv) = mpsc::sync_channel(MAILQUEUE_BOUND);

	thread::spawn(move || {
		while let Ok(mail) = recv.recv() {
			let result = transport.send(mail);
			if result.is_ok() {
				println!("Sent email: {:?}", result);
			} else {
				println!("Error sending email: {:?}", result);
			}
		}
	});

	Mailer {
		from,
		queue: sender,
	}
}
