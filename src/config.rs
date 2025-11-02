use chrono::Duration;
use lettre::message::Mailbox;
use rocket::http::uri::Absolute;
use rocket::serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Config {
	pub admin_email: String,
	pub user_session_seconds: i64,
	pub client_session_seconds: i64,
	pub authorization_token_seconds: i64,
	pub email_confirmation_token_seconds: i64,
	pub secure_token_length: usize,
	pub bcrypt_cost: u32,
	pub base_url: String,
	pub ec_private_key: String,
	pub mail_queue_size: usize,
	pub mail_queue_wait_seconds: u64,
	pub mail_from: String,
	pub mail_server: String,
	pub mail_use_tls: bool,
	pub mail_username: Option<String>,
	pub mail_password: Option<String>,
	pub mail_port: Option<u16>,
	pub mailing_list_name: String,
	pub mailing_list_email: String,
	pub maximum_pending_users: usize,
	pub mailer_role: String,
	pub picture_url_prefix: String,
	pub user_mail_domain: String,
}

impl Config {
	pub fn user_session_duration(&self) -> Duration {
		Duration::seconds(self.user_session_seconds)
	}

	pub fn client_session_duration(&self) -> Duration {
		Duration::seconds(self.client_session_seconds)
	}

	pub fn authorization_token_duration(&self) -> Duration {
		Duration::seconds(self.authorization_token_seconds)
	}

	pub fn email_confirmation_token_duration(&self) -> Duration {
		Duration::seconds(self.email_confirmation_token_seconds)
	}

	pub fn base_url(&self) -> Absolute<'_> {
		Absolute::parse(&self.base_url).expect("valid base_url")
	}

	pub fn picture_url_prefix(&self) -> String {
		self.picture_url_prefix.clone()
	}
}

pub struct AdminEmail(pub Mailbox);
