use crate::errors::{InternalError, Result, ZauthError};
use crate::models::user::User;

use crate::config::Config;
use lettre::{FileTransport, SendableEmail, Transport};
use lettre_email::Email;
use std::env::temp_dir;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const MAIL_QUEUE_BOUND: usize = 32;
const MAIL_QUEUE_WAIT_SECONDS: u64 = 1;

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

	pub fn new(config: &Config) -> Mailer {
		let mut transport = FileTransport::new(temp_dir());

		let from = config.emails_from.clone();

		let (sender, recv) = mpsc::sync_channel(MAIL_QUEUE_BOUND);
		thread::spawn(move || {
			while let Ok(mail) = recv.recv() {
				let result = transport.send(mail);
				if result.is_ok() {
					println!("Sent email: {:?}", result);
				} else {
					println!("Error sending email: {:?}", result);
				}
				// sleep for a while to prevent sending mails too fast
				thread::sleep(Duration::from_secs(MAIL_QUEUE_WAIT_SECONDS));
			}
		});

		Mailer {
			from,
			queue: sender,
		}
	}
}
