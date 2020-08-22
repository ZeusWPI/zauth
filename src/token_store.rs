use crate::util;
use chrono::{DateTime, Duration, Local};
use std::collections::HashMap;
use std::sync::Mutex;

const TOKEN_VALIDITY_SECONDS: i64 = 3600;
const TOKEN_LENGTH: usize = 32;

#[derive(Debug)]
pub struct Token<T> {
	pub token_str: String,
	pub item:      T,
	pub expiry:    DateTime<Local>,
}

#[derive(Debug)]
pub struct TokenStore<T> {
	tokens: Mutex<HashMap<String, Token<T>>>,
}

impl<T> TokenStore<T> {
	pub fn new() -> TokenStore<T> {
		TokenStore {
			tokens: Mutex::new(HashMap::new()),
		}
	}

	fn generate_random_token() -> String {
		util::random_token(TOKEN_LENGTH)
	}

	fn remove_expired_tokens(tokens: &mut HashMap<String, Token<T>>) {
		let now = Local::now();
		tokens.retain(|_key, token| now < token.expiry);
	}

	pub fn create_token(&self, item: T) -> String {
		let tokens: &mut HashMap<String, Token<T>> =
			&mut self.tokens.lock().unwrap();

		Self::remove_expired_tokens(tokens);

		let mut token_str = Self::generate_random_token();
		let mut token = Token {
			item,
			token_str: token_str.clone(),
			expiry: Local::now() + Duration::seconds(TOKEN_VALIDITY_SECONDS),
		};
		while tokens.contains_key(&token_str) {
			token_str = Self::generate_random_token();
			token.token_str = token_str.clone();
		}
		tokens.insert(token_str.clone(), token);
		return token_str;
	}

	pub fn fetch_token(&self, token_str: String) -> Option<Token<T>> {
		let tokens: &mut HashMap<String, Token<T>> =
			&mut self.tokens.lock().unwrap();
		Self::remove_expired_tokens(tokens);
		tokens.remove(&token_str)
	}
}
