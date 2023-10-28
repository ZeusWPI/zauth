use crate::config::Config;
use crate::errors::{InternalError, LaunchError, Result};
use crate::models::client::Client;
use crate::models::user::User;
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use std::fs::File;
use std::io::Read;

pub struct JWTBuilder {
	pub key:    EncodingKey,
	pub header: Header,
}

#[derive(Serialize, Debug)]
pub struct IDToken {
	sub:                String,
	iss:                String,
	aud:                String,
	exp:                i64,
	iat:                i64,
	preferred_username: String,
	email:              String,
}

impl JWTBuilder {
	pub fn new(config: &Config) -> Result<JWTBuilder> {
		let mut file = File::open(&config.ec_private_key)
			.map_err(|err| LaunchError::BadConfigValueType(err.to_string()))?;
		let mut buffer = Vec::new();
		file.read_to_end(&mut buffer)
			.map_err(|err| LaunchError::BadConfigValueType(err.to_string()))?;

		let key = EncodingKey::from_ec_pem(&buffer)
			.map_err(|err| LaunchError::BadConfigValueType(err.to_string()))?;
		let header = Header::new(jsonwebtoken::Algorithm::ES256);

		Ok(JWTBuilder { key, header })
	}

	pub fn encode<T: Serialize>(&self, claims: &T) -> Result<String> {
		Ok(encode(&self.header, claims, &self.key)
			.map_err(InternalError::from)?)
	}

	pub fn encode_id_token(
		&self,
		client: &Client,
		user: &User,
		config: &Config,
	) -> Result<String> {
		let id_token = IDToken {
			sub:      user.id.to_string(),
			iss:      config.base_url().to_string(),
			aud:      client.name.clone(),
			iat:      Utc::now().timestamp(),
			exp:      Utc::now().timestamp() + config.client_session_seconds,
			nickname: user.username.clone(),
			email:    user.email.clone(),
		};
		self.encode(&id_token)
	}
}
