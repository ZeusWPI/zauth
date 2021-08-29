use crate::config::Config;
use crate::errors::{InternalError, LaunchError, Result, ZauthError};
use crate::models::user::User;

use lettre::message::Mailbox;
use lettre::{Address, Message, SmtpTransport, Transport};
use parking_lot::{Condvar, Mutex};
use std::convert::TryInto;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct Mailer {
	from:  Address,
	queue: mpsc::SyncSender<Message>,
}

pub static STUB_MAILER_OUTBOX: (Mutex<Vec<Message>>, Condvar) =
	(Mutex::new(vec![]), Condvar::new());

impl Mailer {
	pub fn build(
		&self,
		user: &User,
		subject: String,
		text: String,
	) -> Result<Message> {
		Ok(Message::builder()
			.to(user.try_into()?)
			.subject(subject)
			.from(Mailbox::new(None, self.from.clone()))
			.body(text)
			.map_err(InternalError::from)?
			.into())
	}

	pub fn try_create(
		&self,
		user: &User,
		subject: String,
		text: String,
	) -> Result<()> {
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
	) -> Result<()> {
		let mail = self.build(user, subject, text)?;

		self.queue
			.send(mail)
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	pub fn new(config: &Config) -> Result<Mailer> {
		let wait = Duration::from_secs(config.mail_queue_wait_seconds);
		let (sender, recv) = mpsc::sync_channel(config.mail_queue_size);

		if config.mail_server == "stub" {
			thread::spawn(Self::stub_sender(wait, recv));
		} else {
			thread::spawn(Self::smtp_sender(wait, recv, &config.mail_server)?);
		}

		Ok(Mailer {
			from:  config
				.mail_from
				.clone()
				.parse()
				.map_err(LaunchError::from)?,
			queue: sender,
		})
	}

	fn stub_sender(
		wait: Duration,
		receiver: Receiver<Message>,
	) -> impl FnOnce() {
		move || {
			while let Ok(mail) = receiver.recv() {
				{
					let (mailbox, condvar) = &STUB_MAILER_OUTBOX;
					println!(
						"\n==> [STUB MAILER] Sending email:\n\n{}\n",
						String::from_utf8_lossy(&mail.formatted())
					);
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
		receiver: Receiver<Message>,
		server: &str,
	) -> Result<impl FnOnce()> {
		let transport = SmtpTransport::relay(server)
			.map_err(LaunchError::from)?
			.build();
		Ok(move || {
			while let Ok(mail) = receiver.recv() {
				let result = transport.send(&mail);
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
