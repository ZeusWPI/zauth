use crate::config::Config;
use crate::util;
use chrono::{DateTime, Duration, Local};
use rocket::tokio::sync::Mutex;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Token<T> {
	pub token_str: String,
	pub item:      T,
	pub expiry:    DateTime<Local>,
}

#[derive(Debug)]
pub struct TokenStore<T> {
	tokens:         Mutex<HashMap<String, Token<T>>>,
	token_validity: Duration,
	token_length:   usize,
}

impl<T> TokenStore<T> {
	pub fn new(config: &Config) -> TokenStore<T> {
		TokenStore {
			tokens:         Mutex::new(HashMap::new()),
			token_validity: config.authorization_token_duration(),
			token_length:   config.secure_token_length,
		}
	}

	fn generate_random_token(&self) -> String {
		util::random_token(self.token_length)
	}

	fn remove_expired_tokens(tokens: &mut HashMap<String, Token<T>>) {
		let now = Local::now();
		tokens.retain(|_key, token| now < token.expiry);
	}

	pub async fn create_token(&self, item: T) -> String {
		let mut tokens = self.tokens.lock().await;

		Self::remove_expired_tokens(&mut tokens);

		let mut token_str = self.generate_random_token();
		let mut token = Token {
			item,
			token_str: token_str.clone(),
			expiry: Local::now() + self.token_validity,
		};
		while tokens.contains_key(&token_str) {
			token_str = self.generate_random_token();
			token.token_str = token_str.clone();
		}
		tokens.insert(token_str.clone(), token);
		return token_str;
	}

	pub async fn fetch_token(&self, token_str: String) -> Option<Token<T>> {
		let mut tokens = &mut self.tokens.lock().await;
		Self::remove_expired_tokens(&mut tokens);
		tokens.remove(&token_str)
	}
}
