use crate::config::Config;
use crate::errors::{InternalError, LaunchError, Result};
use crate::models::client::Client;
use crate::models::user::User;
use base64::engine::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use jsonwebtoken::jwk::{
	CommonParameters, EllipticCurveKeyParameters, Jwk, JwkSet,
};
use jsonwebtoken::{EncodingKey, Header, encode};
use openssl::bn::{BigNum, BigNumContext};
use openssl::ec::EcKey;
use serde::Serialize;
use std::fs::File;
use std::io::Read;

pub struct JWTBuilder {
	pub key: EncodingKey,
	pub header: Header,
	pub jwks: JwkSet,
}

#[derive(Serialize, Debug)]
struct IDToken {
	sub: String,
	iss: String,
	aud: String,
	exp: i64,
	iat: i64,
	preferred_username: String,
	email: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	roles: Option<Vec<String>>,
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
		let header = Header::new(jsonwebtoken::Algorithm::ES384);

		let private_key = EcKey::private_key_from_pem(&buffer)
			.map_err(|err| LaunchError::BadConfigValueType(err.to_string()))?;

		let mut ctx: BigNumContext = BigNumContext::new().unwrap();
		let public_key = private_key.public_key();
		let mut x = BigNum::new().unwrap();
		let mut y = BigNum::new().unwrap();
		public_key
			.affine_coordinates(private_key.group(), &mut x, &mut y, &mut ctx)
			.expect("x,y coordinates");

		let jwk = Jwk {
			common: CommonParameters {
				public_key_use: Some(
					jsonwebtoken::jwk::PublicKeyUse::Signature,
				),
				key_algorithm: Some(jsonwebtoken::jwk::KeyAlgorithm::ES384),
				key_operations: None,
				key_id: None,
				x509_url: None,
				x509_chain: None,
				x509_sha1_fingerprint: None,
				x509_sha256_fingerprint: None,
			},
			algorithm: jsonwebtoken::jwk::AlgorithmParameters::EllipticCurve(
				EllipticCurveKeyParameters {
					key_type: jsonwebtoken::jwk::EllipticCurveKeyType::EC,
					curve: jsonwebtoken::jwk::EllipticCurve::P384,
					x: URL_SAFE_NO_PAD.encode(x.to_vec()),
					y: URL_SAFE_NO_PAD.encode(y.to_vec()),
				},
			),
		};

		Ok(JWTBuilder {
			key,
			header,
			jwks: JwkSet {
				keys: Vec::from([jwk]),
			},
		})
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
		roles: Option<Vec<String>>,
	) -> Result<String> {
		let id_token = IDToken {
			sub: user.id.to_string(),
			iss: config.base_url().to_string(),
			aud: client.name.clone(),
			iat: Utc::now().timestamp(),
			exp: Utc::now().timestamp() + config.client_session_seconds,
			preferred_username: user.username.clone(),
			email: user.email.clone(),
			roles,
		};
		self.encode(&id_token)
	}
}
