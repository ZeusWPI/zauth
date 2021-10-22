use crate::config::{AdminEmail, Config};
use crate::errors::{InternalError, Result, ZauthError};
use crate::mailer::Mailer;
use crate::models::user::User;

use askama::Template;
use lettre::message::Mailbox;
use rocket::tokio::sync::mpsc;
use rocket::tokio::sync::mpsc::UnboundedReceiver;

#[derive(Clone)]
pub struct Hooker {
	queue: mpsc::UnboundedSender<User>,
}

impl Hooker {
	pub fn new(
		config: &Config,
		mailer: &Mailer,
		admin_email: &AdminEmail,
	) -> Result<Hooker> {
		// Webhooks are only triggered by admin actions, so no need to worry
		// about abuse
		let (sender, recv) = mpsc::unbounded_channel();

		if let Some(url) = &config.webhook_url {
			rocket::tokio::spawn(Self::http_sender(
				url.into(),
				recv,
				admin_email.0.clone(),
				mailer.clone(),
			));
		} else {
			rocket::tokio::spawn(Self::stub_sender(recv));
		}

		Ok(Hooker { queue: sender })
	}

	pub async fn user_approved(&self, user: &User) -> Result<()> {
		self.queue
			.send(user.clone())
			.map_err(|e| ZauthError::from(InternalError::from(e)))
	}

	fn stub_sender(
		mut receiver: UnboundedReceiver<User>,
	) -> impl std::future::Future<Output = impl Send + 'static> {
		async move {
			// no URL configured, so we just drop the received users
			while let Some(_) = receiver.recv().await {}
		}
	}

	fn http_sender(
		url: String,
		mut receiver: UnboundedReceiver<User>,
		admin_email: Mailbox,
		mailer: Mailer,
	) -> impl std::future::Future<Output = impl Send + 'static> {
		async move {
			let client = reqwest::Client::new();
			while let Some(user) = receiver.recv().await {
				if let Err(err) =
					Self::do_send(&client, &url, &admin_email, &mailer, &user)
						.await
				{
					println!("Error sending webhook: {:?}", err);
				}
			}
		}
	}

	async fn do_send(
		client: &reqwest::Client,
		url: &str,
		admin_email: &Mailbox,
		mailer: &Mailer,
		user: &User,
	) -> Result<()> {
		if let Err(err) = client.post(url.clone()).json(&user).send().await {
			mailer
				.create(
					admin_email.clone(),
					String::from("[Zauth] Confirm webhook failed"),
					template!(
						"mails/registration_webhook_failed.txt";
						name: String = user.username.to_string(),
						err: String = format!("{:?}", err),
					)
					.render()
					.map_err(InternalError::from)?,
				)
				.await?;
		}
		Ok(())
	}
}
