use crate::config::Config;
use crate::errors::{InternalError, LaunchError, Result, ZauthError};

use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, Message, SmtpTransport, Transport};
use parking_lot::{Condvar, Mutex};
use rocket::Config as RocketConfig;
use rocket::tokio::sync::mpsc::Receiver;
use rocket::tokio::sync::mpsc::{self, UnboundedReceiver};
use rocket::tokio::time::sleep;
use std::convert::TryInto;
use std::time::Duration;

#[derive(Clone)]
pub struct Mailer {
	from: Address,
	queue: mpsc::Sender<Message>,
	mailinglist_queue: mpsc::UnboundedSender<Message>,
}

pub static STUB_MAILER_OUTBOX: (Mutex<Vec<Message>>, Condvar) =
	(Mutex::new(vec![]), Condvar::new());

impl Mailer {
	pub fn build<E: Into<ZauthError>, M: TryInto<Mailbox, Error = E>>(
		&self,
		receiver: M,
		subject: String,
		text: String,
		content_type: ContentType,
	) -> Result<Message> {
		Ok(Message::builder()
			.to(receiver.try_into().map_err(|e| e.into())?)
			.header(content_type)
			.subject(subject)
			.from(Mailbox::new(None, self.from.clone()))
			.body(text)
			.map_err(InternalError::from)?
			.into())
	}

	/// Send an email, but fail when the mail queue is full.
	///
	/// Use this method for less important emails where abuse may be possible.
	pub fn try_create<E: Into<ZauthError>, M: TryInto<Mailbox, Error = E>>(
		&self,
		receiver: M,
		subject: String,
		text: String,
		content_type: ContentType,
	) -> Result<()> {
		let email = self.build(receiver, subject, text, content_type)?;
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
		content_type: ContentType,
	) -> Result<()> {
		let mail = self.build(receiver, subject, text, content_type)?;

		self.queue
			.send(mail)
			.await
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	/// Send an email using the mailinglist queue
	pub fn create_for_mailinglist<
		E: Into<ZauthError>,
		M: TryInto<Mailbox, Error = E>,
	>(
		&self,
		receiver: M,
		subject: String,
		text: String,
		content_type: ContentType,
	) -> Result<()> {
		let mail = self.build(receiver, subject, text, content_type)?;

		self.mailinglist_queue
			.send(mail)
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	pub fn new(config: &Config) -> Result<Mailer> {
		let wait = Duration::from_secs(config.mail_queue_wait_seconds);
		let (sender, recv) = mpsc::channel(config.mail_queue_size);
		let (list_sender, list_recv) = mpsc::unbounded_channel();

		if config.mail_server == "stub" {
			rocket::tokio::spawn(Self::stub_sender(wait, recv));
			rocket::tokio::spawn(Self::unbounded_stub_sender(wait, list_recv));
		} else {
			let transport = Self::build_smtp_transport(config)?;
			rocket::tokio::spawn(Self::smtp_sender(wait, recv, transport));
			let transport = Self::build_smtp_transport(config)?;
			rocket::tokio::spawn(Self::unbounded_smtp_sender(
				wait, list_recv, transport,
			));
		}

		Ok(Mailer {
			from: config
				.mail_from
				.clone()
				.parse()
				.map_err(LaunchError::from)?,
			queue: sender,
			mailinglist_queue: list_sender,
		})
	}

	fn build_smtp_transport(config: &Config) -> Result<SmtpTransport> {
		let rocket_config: RocketConfig =
			RocketConfig::figment().extract().unwrap();
		let is_prod = rocket_config.profile == "release";

		let port = config.mail_port.unwrap_or(if config.mail_use_tls {
			465
		} else {
			25
		});

		let mut transport_builder = if config.mail_use_tls {
			SmtpTransport::relay(&config.mail_server)
				.map_err(LaunchError::from)?
				.port(port)
		} else {
			SmtpTransport::builder_dangerous(&config.mail_server).port(port)
		};

		if let (Some(username), Some(password)) =
			(&config.mail_username, &config.mail_password)
		{
			if !config.mail_use_tls && is_prod {
				return Err(ZauthError::Launch(
						LaunchError::BadConfigValueType(
							"Can't use SMTP authentication without TLS in production".to_owned(),
						),
					));
			}
			transport_builder = transport_builder.credentials(
				Credentials::new(username.clone(), password.clone()),
			);
		}

		Ok(transport_builder.build())
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

	fn unbounded_stub_sender(
		wait: Duration,
		mut receiver: UnboundedReceiver<Message>,
	) -> impl std::future::Future<Output = impl Send + 'static> {
		async move {
			while let Some(mail) = receiver.recv().await {
				{
					let (mailbox, condvar) = &STUB_MAILER_OUTBOX;
					eprintln!(
						"\n==> [UNBOUNDED STUB MAILER] Sending mailinglist \
						 email:\n\n{}\n",
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
		transport: SmtpTransport,
	) -> impl std::future::Future<Output = impl Send + 'static + use<>> + use<>
	{
		async move {
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
		}
	}

	fn unbounded_smtp_sender(
		wait: Duration,
		mut receiver: UnboundedReceiver<Message>,
		transport: SmtpTransport,
	) -> impl std::future::Future<Output = impl Send + 'static + use<>> + use<>
	{
		async move {
			while let Some(mail) = receiver.recv().await {
				let result = transport.send(&mail);
				if result.is_ok() {
					println!("Sent mailinglist email: {:?}", result);
				} else {
					println!("Error sending mailinglist email: {:?}", result);
				}
				// sleep for a while to prevent sending mails too fast
				sleep(wait).await;
			}
		}
	}
}
