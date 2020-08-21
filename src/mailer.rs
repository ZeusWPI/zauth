use crate::errors::{InternalError, Result};
use crate::models::user::User;

use lettre::{SendableEmail, SmtpTransport, Transport};
use lettre_email::Email;
use std::sync::mpsc;
use std::thread;

pub struct Mailer {
	from:  String,
	queue: mpsc::Sender<SendableEmail>,
}

impl Mailer {
	pub fn create(
		self,
		user: &User,
		subject: String,
		text: String,
	) -> Result<()>
	{
		let mail = Email::builder()
			.to(user)
			.subject(subject)
			.from(self.from)
			.text(text)
			.build()
			.map_err(InternalError::from)?;
		self.queue.send(mail.into()).map_err(InternalError::from)?;
		Ok(())
	}
}

pub fn create_queue(from: String, mut transport: SmtpTransport) -> Mailer {
	let (queue, recv) = mpsc::channel();

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

	Mailer { from, queue }
}
