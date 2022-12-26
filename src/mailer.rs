use crate::config::Config;
use crate::errors::{InternalError, LaunchError, Result, ZauthError};

use lettre::message::Mailbox;
use lettre::{Address, Message, SmtpTransport, Transport};
use parking_lot::{Condvar, Mutex};
use rocket::tokio::sync::mpsc;
use rocket::tokio::sync::mpsc::Receiver;
use rocket::tokio::time::sleep;
use std::convert::TryInto;
use std::time::Duration;

#[derive(Clone)]
pub struct Mailer {
	from:  Address,
	queue: mpsc::Sender<Message>,
}

pub static STUB_MAILER_OUTBOX: (Mutex<Vec<Message>>, Condvar) =
	(Mutex::new(vec![]), Condvar::new());

impl Mailer {
	pub fn build<E: Into<ZauthError>, M: TryInto<Mailbox, Error = E>>(
		&self,
		receiver: M,
		subject: String,
		text: String,
	) -> Result<Message> {
		Ok(Message::builder()
			.to(receiver.try_into().map_err(|e| e.into())?)
			.subject(subject)
			.from(Mailbox::new(None, self.from.clone()))
			.body(text)
			.map_err(InternalError::from)?
			.into())
	}

	/// Build an email with a list of addresses in bcc
	pub fn build_with_bcc<E, M, R, B>(
		&self,
		receiver: M,
		bcc: Vec<B>,
		subject: String,
		text: String,
	) -> Result<Message>
	where
		E: Into<ZauthError>,
		M: TryInto<Mailbox, Error = E>,
		R: Into<ZauthError>,
		B: TryInto<Mailbox, Error = R>,
	{
		let mut builder = Message::builder()
			.to(receiver.try_into().map_err(|e| e.into())?)
			.keep_bcc()
			.subject(subject)
			.from(Mailbox::new(None, self.from.clone()));

		for addr in bcc {
			builder = builder.bcc(addr.try_into().map_err(|e| e.into())?);
		}

		Ok(builder.body(text).map_err(InternalError::from)?.into())
	}

	/// Send an email, but fail when the mail queue is full.
	///
	/// Use this method for less important emails where abuse may be possible.
	pub fn try_create<E: Into<ZauthError>, M: TryInto<Mailbox, Error = E>>(
		&self,
		receiver: M,
		subject: String,
		text: String,
	) -> Result<()> {
		let email = self.build(receiver, subject, text)?;
		self.queue
			.try_send(email)
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	/// Try to send an email with a list of addresses in BCC
	pub fn try_create_with_bcc<
		E: Into<ZauthError>,
		M: TryInto<Mailbox, Error = E>,
	>(
		&self,
		receiver_name: &str,
		receiver_email: &str,
		bcc: Vec<M>,
		subject: String,
		text: String,
	) -> Result<()> {
		let receiver = Mailbox::new(
			Some(receiver_name.to_string()),
			receiver_email.parse().map_err(InternalError::from)?,
		);

		let email = self.build_with_bcc(receiver, bcc, subject, text)?;

		self.queue
			.try_send(email)
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	/// Send an email, but block when the mail queue is full.
	///
	/// Use this method only for important emails where the possibility for
	/// abuse is minimal.
	pub async fn create<E: Into<ZauthError>, M: TryInto<Mailbox, Error = E>>(
		&self,
		receiver: M,
		subject: String,
		text: String,
	) -> Result<()> {
		let mail = self.build(receiver, subject, text)?;

		self.queue
			.send(mail)
			.await
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	pub fn new(config: &Config) -> Result<Mailer> {
		let wait = Duration::from_secs(config.mail_queue_wait_seconds);
		let (sender, recv) = mpsc::channel(config.mail_queue_size);

		if config.mail_server == "stub" {
			rocket::tokio::spawn(Self::stub_sender(wait, recv));
		} else {
			rocket::tokio::spawn(Self::smtp_sender(
				wait,
				recv,
				&config.mail_server,
			)?);
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
		mut receiver: Receiver<Message>,
	) -> impl std::future::Future<Output = impl Send + 'static> {
		async move {
			while let Some(mail) = receiver.recv().await {
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
				sleep(wait).await;
			}
		}
	}

	fn smtp_sender(
		wait: Duration,
		mut receiver: Receiver<Message>,
		server: &str,
	) -> Result<impl std::future::Future<Output = impl Send + 'static>> {
		let transport = SmtpTransport::builder_dangerous(server).build();
		Ok(async move {
			while let Some(mail) = receiver.recv().await {
				let result = transport.send(&mail);
				if result.is_ok() {
					println!("Sent email: {:?}", result);
				} else {
					println!("Error sending email: {:?}", result);
				}
				// sleep for a while to prevent sending mails too fast
				sleep(wait).await;
			}
		})
	}
}
