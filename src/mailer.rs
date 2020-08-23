use crate::config::Config;
use crate::errors::{InternalError, LaunchError, Result, ZauthError};
use crate::models::user::User;

use lettre::{SendableEmail, SmtpClient, Transport};
use lettre_email::Email;
use parking_lot::{Condvar, Mutex};
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct Mailer {
	from:  String,
	queue: mpsc::SyncSender<SendableEmail>,
}

pub static STUB_MAILER_OUTBOX: (Mutex<Vec<SendableEmail>>, Condvar) =
	(Mutex::new(vec![]), Condvar::new());

impl Mailer {
	pub fn build(
		&self,
		user: &User,
		subject: String,
		text: String,
	) -> Result<SendableEmail>
	{
		Ok(Email::builder()
			.to(user)
			.subject(subject)
			.from(self.from.clone())
			.text(text)
			.build()
			.map_err(InternalError::from)?
			.into())
	}

	pub fn try_create(
		&self,
		user: &User,
		subject: String,
		text: String,
	) -> Result<()>
	{
		let email = self.build(user, subject, text)?;
		self.queue
			.try_send(email)
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	pub fn create(
		&self,
		user: &User,
		subject: String,
		text: String,
	) -> Result<()>
	{
		let mail = self.build(user, subject, text)?;

		self.queue
			.send(mail)
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	pub fn new(config: &Config) -> Result<Mailer> {
		let wait = Duration::from_secs(config.mail_queue_wait_seconds);
		let (sender, recv) = mpsc::sync_channel(config.mail_queue_size);

		match config.mail_server {
			"stub" => thread::spawn(Self::stub_sender(wait, recv)),
			server => thread::spawn(Self::smtp_sender(wait, recv, server)?),
		};

		Ok(Mailer {
			from:  config.mail_from.clone(),
			queue: sender,
		})
	}

	fn stub_sender(
		wait: Duration,
		receiver: Receiver<SendableEmail>,
	) -> impl FnOnce()
	{
		move || {
			while let Ok(mail) = receiver.recv() {
				println!("Email received");
				{
					let (mailbox, condvar) = &STUB_MAILER_OUTBOX;
					mailbox.lock().push(mail);
					condvar.notify_all();
				}

				// sleep for a while to prevent sending mails too fast
				thread::sleep(wait);
			}
		}
	}

	fn smtp_sender(
		wait: Duration,
		receiver: Receiver<SendableEmail>,
		server: &str,
	) -> Result<impl FnOnce()>
	{
		let mut transport = SmtpClient::new_simple(server)
			.map_err(|e| ZauthError::from(LaunchError::from(e)))?
			.transport();
		Ok(move || {
			while let Ok(mail) = receiver.recv() {
				let result = transport.send(mail);
				if result.is_ok() {
					println!("Sent email: {:?}", result);
				} else {
					println!("Error sending email: {:?}", result);
				}
				// sleep for a while to prevent sending mails too fast
				thread::sleep(wait);
			}
		})
	}
}
