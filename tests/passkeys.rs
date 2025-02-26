#![feature(async_closure)]

extern crate diesel;
extern crate rocket;

use chrono::Local;
use common::HttpClient;
use rocket::form::validate::Contains;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::State;
use serde_json::json;
use webauthn_rs::prelude::DiscoverableAuthentication;
use webauthn_rs::prelude::Uuid;
use zauth::errors::Either;
use zauth::models::passkey::NewPassKey;
use zauth::models::passkey::PassKey;
use zauth::models::user::NewUser;
use zauth::models::user::User;
use zauth::webauthn::WebAuthnStore;

mod common;

#[rocket::async_test]
async fn register_passkey_as_visitor() {
	common::as_visitor(async move |http_client: HttpClient, _db| {
		let response = http_client
			.post("/webauthn/start_register")
			.header(ContentType::JSON)
			.body("true")
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Unauthorized);
	})
	.await;
}

#[rocket::async_test]
async fn list_passkeys_as_visitor() {
	common::as_visitor(async move |http_client: HttpClient, _db| {
		let response = http_client.get("/passkeys").dispatch().await;

		assert_eq!(response.status(), Status::Unauthorized);
	})
	.await;
}

#[rocket::async_test]
async fn list_passkeys_as_user() {
	common::as_user(async move |http_client: HttpClient, _db, _user: User| {
		let response = http_client.get("/passkeys").dispatch().await;

		assert_eq!(response.status(), Status::Ok);
	})
	.await;
}

#[rocket::async_test]
async fn passkey_login() {
	common::as_visitor(async move |http_client: HttpClient, db| {
		let user = User::create(
			NewUser {
				username:    String::from("user"),
				password:    String::from("password"),
				full_name:   String::from("name"),
				email:       String::from("example@example.org"),
				ssh_key:     None,
				not_a_robot: true,
			},
			common::BCRYPT_COST,
			&db,
		).await.expect("user create");

		let passkey_json = json!({
		    "cred":{"cred_id":"6syH3sLIgnj_C7vIkxHP-2QeOWaSPGZ8ybkKytd5c4WpR7ufzzgrTDmsP5rKxmHT",
		        "cred":{"type_":"ES256","key":{"EC_EC2":{"curve":"SECP256R1","x":"6syH3sLIgnj_C7vIk3MbihBUY43vqIPYVlvOdrpCMfY","y":"9NUqLu0rj_LLR1zaqbrQvYDUY7KdCtTWqXgS0zcuHN0"}}},
		        "counter":40,"transports":null,"user_verified":true,"backup_eligible":false,"backup_state":false,"registration_policy":"required",
		        "extensions":{"cred_protect":"Ignored","hmac_create_secret":"NotRequested","appid":"NotRequested","cred_props":"Ignored"},"attestation":{"data":"None","metadata":"None"},
		        "attestation_format":"none"
		     }});

		PassKey::create(NewPassKey{
			user_id: user.id,
			name: String::from("test"),
			cred: serde_json::from_value(passkey_json).expect("valid passkey")
		}, &db).await.expect("passkey create");

		let uuid = Uuid::from_u128(user.id as u128);

		let user_handle = base64::encode_config(uuid.as_bytes(), base64::URL_SAFE_NO_PAD);

		let state: &State<WebAuthnStore> = State::get(http_client.rocket()).expect("managed `Webauthn`");
		let id = Local::now();
		let id_json = json!(id);
		let auth_json = json!({"ast":{"credentials":[],"policy":"required","challenge":"0b82YlQK81wCveDefnunqVoTz3PERzTOUTFXfqOBKL0","allow_backup_eligible_upgrade":false}});
		let auth_state: DiscoverableAuthentication = serde_json::from_value(auth_json).expect("valid auth_state");

		state.add_authentication(id, Either::Left(auth_state.clone())).await;

		// Test valid credential
		let signature = "MEYCIQCfvrbmI2Kn2O27qQCdjkoqYSShu9x1ngKg73svLH88wgIhAMbbOI19BnY4ij79Llb0U2RL0cS2MLjkPohdpm7_nlkr";
		let credential_json = json!(
		    {"id":"6syH3sLIgnj_C7vIkxHP-2QeOWaSPGZ8ybkKytd5c4WpR7ufzzgrTDmsP5rKxmHT",
		    "rawId":"6syH3sLIgnj_C7vIkxHP-2QeOWaSPGZ8ybkKytd5c4WpR7ufzzgrTDmsP5rKxmHT",
		    "response":{"authenticatorData":"SZYN5YgOjGh0NBcPZHZgW4_krrmihjLHmVzzuoMdl2MFAAAAVg",
		    "clientDataJSON":"eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoiMGI4MllsUUs4MXdDdmVEZWZudW5xVm9UejNQRVJ6VE9VVEZYZnFPQktMMCIsIm9yaWdpbiI6Imh0dHA6Ly9sb2NhbGhvc3Q6ODAwMCJ9",
		    "signature": signature,"userHandle": user_handle},"extensions":{"appid":null,"hmac_get_secret":null},"type":"public-key"}
		);


		let response = http_client.post("/webauthn/finish_auth").header(ContentType::Form).body(format!("id={}&credential={}", urlencoding::encode(&id_json.to_string()), urlencoding::encode(&credential_json.to_string()))).dispatch().await;

		assert_eq!(response.status(), Status::SeeOther);

		state.add_authentication(id, Either::Left(auth_state)).await;

		// Test invalid credential
		let invalid_signature = "MEYCIQCfvrbmI2Kn2O27qQCdjkoqYSShu9x1ngKg73svLH89wgIhAMbbOI19BnY4ij79Llb0U2RL0cS2MLjkPohdpm7_nlkr";
		let credential_json = json!(
		    {"id":"6syH3sLIgnj_C7vIkxHP-2QeOWaSPGZ8ybkKytd5c4WpR7ufzzgrTDmsP5rKxmHT",
		    "rawId":"6syH3sLIgnj_C7vIkxHP-2QeOWaSPGZ8ybkKytd5c4WpR7ufzzgrTDmsP5rKxmHT",
		    "response":{"authenticatorData":"SZYN5YgOjGh0NBcPZHZgW4_krrmihjLHmVzzuoMdl2MFAAAAVg",
		    "clientDataJSON":"eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoiMGI4MllsUUs4MXdDdmVEZWZudW5xVm9UejNQRVJ6VE9VVEZYZnFPQktMMCIsIm9yaWdpbiI6Imh0dHA6Ly9sb2NhbGhvc3Q6ODAwMCJ9",
		    "signature": invalid_signature,"userHandle": user_handle},"extensions":{"appid":null,"hmac_get_secret":null},"type":"public-key"}
		);
		let response = http_client.post("/webauthn/finish_auth").header(ContentType::Form).body(format!("id={}&credential={}", urlencoding::encode(&id_json.to_string()), urlencoding::encode(&credential_json.to_string()))).dispatch().await;

		assert_eq!(response.status(), Status::Ok);
		assert!(response.into_string().await.contains("Passkey authentication failed"))

	}).await;
}
