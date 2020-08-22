use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub fn random_token(token_length: usize) -> String {
	thread_rng()
		.sample_iter(&Alphanumeric)
		.take(token_length)
		.collect()
}
