use crate::config::{AdminEmail, Config};
use crate::errors::{InternalError, Result};
use crate::mailer::Mailer;
use crate::models::user::User;

use askama::Template;
use lettre::message::Mailbox;

#[derive(Clone)]
pub struct Hooker {
	admin_email: Mailbox,
	url:         String,
	mailer:      Mailer,
}

impl Hooker {
	pub fn new(
		config: &Config,
		mailer: &Mailer,
		admin_email: &AdminEmail,
	) -> Result<Hooker> {
		Ok(Hooker {
			admin_email: admin_email.0.clone(),
			url:         config.webhook_url.clone(),
			mailer:      mailer.clone(),
		})
	}

	pub async fn user_approved(&self, user: &User) -> Result<()> {
		let client = reqwest::Client::new();
		if let Err(err) = client.post(self.url.clone()).json(user).send().await
		{
			self.mailer
				.create(
					self.admin_email.clone(),
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
