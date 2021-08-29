use rocket::serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Config {
	pub authorization_token_validity_seconds: usize,
	pub secure_token_length: usize,
	pub bcrypt_cost: u32,
	pub base_url: String,
	pub mail_queue_size: usize,
	pub mail_queue_wait_seconds: u64,
	pub mail_from: String,
	pub mail_server: String,
}
