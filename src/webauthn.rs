use std::collections::HashMap;

use chrono::{DateTime, Local, TimeDelta};
use rocket::tokio::sync::Mutex;
use webauthn_rs::{
	prelude::{
		DiscoverableAuthentication, PasskeyAuthentication, PasskeyRegistration,
		Url,
	},
	Webauthn, WebauthnBuilder,
};

use crate::{config::Config, errors::Either};

type Authentication =
	Either<DiscoverableAuthentication, (PasskeyAuthentication, i32)>;

pub struct WebAuthnStore {
	registrations:   Mutex<HashMap<i32, PasskeyRegistration>>,
	authentications: Mutex<HashMap<DateTime<Local>, Authentication>>,
	pub webauthn:    Webauthn,
}

impl WebAuthnStore {
	pub fn new(config: &Config) -> Self {
		let base_url = Url::parse(&config.base_url).expect("Invalid base url");
		let webauthn_builder = WebauthnBuilder::new(
			base_url.domain().expect("No domain in base_url"),
			&base_url,
		)
		.expect("Invalid webauthn configuration");
		let webauthn = webauthn_builder
			.build()
			.expect("Invalid webauthn configuration");

		WebAuthnStore {
			registrations: Mutex::new(HashMap::new()),
			authentications: Mutex::new(HashMap::new()),
			webauthn,
		}
	}

	pub async fn add_registration(
		&self,
		user_id: i32,
		reg_state: PasskeyRegistration,
	) {
		let registrations = &mut self.registrations.lock().await;
		registrations.insert(user_id, reg_state);
	}

	pub async fn fetch_registration(
		&self,
		user_id: i32,
	) -> Option<PasskeyRegistration> {
		let registrations = &mut self.registrations.lock().await;
		registrations.remove(&user_id)
	}

	fn remove_expired_auths(
		auths: &mut HashMap<DateTime<Local>, Authentication>,
	) {
		let expiration = Local::now() - TimeDelta::minutes(2);
		auths.retain(|key, _auth| expiration < *key);
	}

	pub async fn add_authentication(
		&self,
		user_id: DateTime<Local>,
		auth_state: Authentication,
	) {
		let mut auths = self.authentications.lock().await;
		Self::remove_expired_auths(&mut auths);
		auths.insert(user_id, auth_state);
	}

	pub async fn fetch_authentication(
		&self,
		user_id: DateTime<Local>,
	) -> Option<Authentication> {
		let mut auths = self.authentications.lock().await;
		Self::remove_expired_auths(&mut auths);
		auths.remove(&user_id)
	}
}
