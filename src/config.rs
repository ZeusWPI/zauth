use pwhash::bcrypt;
use std::convert::TryFrom;
use toml::de::Error;

macro_rules! config_options {
    ($($name:ident: $type:ty = $default:expr),+$(,)?) => {
    	#[derive(Debug, Clone)]
    	pub struct Config {
			$(
				pub $name: $type,
			)+
    	}
    	impl TryFrom<&rocket::Config> for Config {
			type Error = Error;

			fn try_from(config: &rocket::Config) -> Result<Self, Self::Error> {
				Ok(Config {
					$(
						$name: {
							if let Some(value) = config.extras.get("$name") {
							 	value.to_owned().try_into()?
							} else {
								$default
							}
						},
					)+
				})
			}
		}
    };
}

config_options!(
	authorization_token_validity_seconds: usize = 300,
	secure_token_length: usize = 64,
	bcrypt_cost: u32 = bcrypt::DEFAULT_COST,
	mail_queue_size: usize = 32,
	mail_queue_wait_seconds: u64 = 1,
	mail_from: String = String::from("zauth@zeus.ugent.be"),
	mail_server: &'static str = "stub"
);
