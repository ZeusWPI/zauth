use chrono::{DateTime, Local};
use rocket::http::{CookieJar, Status};
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket::{serde::json::Json, State};
use webauthn_rs::prelude::*;
use webauthn_rs_proto::{
	AuthenticatorSelectionCriteria, ResidentKeyRequirement,
	UserVerificationPolicy,
};

use crate::config::Config;
use crate::controllers::pages_controller::rocket_uri_macro_home_page;
use crate::ephemeral::session::{
	stored_redirect_or, SessionCookie, UserSession,
};
use crate::errors::{
	AuthenticationError, Either, LoginError, Result, ZauthError,
};
use crate::models::passkey::{NewPassKey, PassKey};
use crate::models::session::Session;
use crate::models::user::User;
use crate::views::accepter::Accepter;
use crate::webauthn::WebAuthnStore;
use crate::DbConn;

#[post("/webauthn/start_register", format = "json", data = "<residential>")]
pub async fn start_register(
	session: UserSession,
	webauthn_store: &State<WebAuthnStore>,
	residential: Json<bool>,
	db: DbConn,
) -> Result<Json<CreationChallengeResponse>> {
	let authenticator_criteria = AuthenticatorSelectionCriteria {
		authenticator_attachment: None,
		resident_key:             if *residential {
			Some(ResidentKeyRequirement::Required)
		} else {
			Some(ResidentKeyRequirement::Discouraged)
		},
		require_resident_key:     *residential,
		user_verification:        UserVerificationPolicy::Required,
	};

	let exclude = PassKey::find_credentials(session.user.id, &db)
		.await?
		.iter()
		.map(|cred| cred.cred_id().clone())
		.collect();

	match webauthn_store.webauthn.start_passkey_registration(
		Uuid::from_u128(session.user.id as u128),
		&session.user.username,
		&session.user.username,
		Some(exclude),
	) {
		Ok((mut ccr, reg_state)) => {
			webauthn_store
				.add_registration(session.user.id, reg_state)
				.await;

			ccr.public_key.authenticator_selection =
				Some(authenticator_criteria);
			Ok(Json(ccr))
		},
		Err(e) => Err(e.into()),
	}
}

#[derive(Deserialize)]
pub struct PassKeyRegistration {
	credential: RegisterPublicKeyCredential,
	name:       String,
}

#[post("/webauthn/finish_register", format = "json", data = "<reg>")]
pub async fn finish_register<'r>(
	session: UserSession,
	webauthn_store: &State<WebAuthnStore>,
	reg: Json<PassKeyRegistration>,
	db: DbConn,
) -> Result<Either<Redirect, impl Responder<'r, 'static>>> {
	let reg_state =
		match webauthn_store.fetch_registration(session.user.id).await {
			Some(registration) => registration,
			None => {
				return Err(ZauthError::WebauthnError(
					WebauthnError::ChallengeNotFound,
				))
			},
		};

	match webauthn_store
		.webauthn
		.finish_passkey_registration(&reg.credential, &reg_state)
	{
		Ok(pk) => {
			let passkey = NewPassKey {
				user_id: session.user.id,
				name:    reg.name.clone(),
				cred:    pk,
			};

			PassKey::create(passkey, &db).await?;
			Ok(Either::Left(Redirect::to(uri!(list_passkeys))))
		},
		Err(e) => Ok(Either::Right(template! {
			"passkeys/new_passkey.html";
			current_user: User = session.user,
			errors: Option<String> = Some(e.to_string()),
		})),
	}
}

#[post("/webauthn/start_auth", format = "json", data = "<username>")]
pub async fn start_authentication(
	webauthn_store: &State<WebAuthnStore>,
	username: Json<Option<String>>,
	db: DbConn,
) -> Result<Json<(DateTime<Local>, RequestChallengeResponse)>> {
	let now = Local::now();

	let user_opt = if let Some(name) = username.into_inner() {
		User::find_by_username(name, &db).await.ok()
	} else {
		None
	};

	match user_opt {
		Some(user) => {
			let creds: Vec<Passkey> =
				PassKey::find_credentials(user.id, &db).await?;

			match webauthn_store
				.webauthn
				.start_passkey_authentication(creds.as_slice())
			{
				Ok((rcr, auth_state)) => {
					webauthn_store
						.add_authentication(
							now,
							Either::Right((auth_state, user.id)),
						)
						.await;
					Ok(Json((now, rcr)))
				},
				Err(e) => Err(e.into()),
			}
		},
		None => {
			match webauthn_store.webauthn.start_discoverable_authentication() {
				Ok((rcr, auth_state)) => {
					webauthn_store
						.add_authentication(now, Either::Left(auth_state))
						.await;
					Ok(Json((now, rcr)))
				},
				Err(e) => Err(e.into()),
			}
		},
	}
}

#[derive(Deserialize, Debug)]
pub struct PassKeyAuthentication {
	id:         DateTime<Local>,
	username:   Option<String>,
	credential: Option<PublicKeyCredential>,
}

async fn authenticate(
	webauthn_store: &WebAuthnStore,
	auth: PassKeyAuthentication,
	db: &DbConn,
) -> Result<User> {
	let (result, user) = match webauthn_store
		.fetch_authentication(auth.id)
		.await
	{
		Some(Either::Left(discoverable_state)) => {
			let credential = auth.credential.ok_or(ZauthError::LoginError(
				LoginError::PasskeyDiscoverableError,
			))?;
			let (uuid, _) = webauthn_store
				.webauthn
				.identify_discoverable_authentication(&credential)?;

			let user = User::find(uuid.as_u128() as i32, db).await?;

			let creds: Vec<DiscoverableKey> =
				PassKey::find_credentials(user.id, db)
					.await?
					.iter()
					.map(DiscoverableKey::from)
					.collect();

			webauthn_store
				.webauthn
				.finish_discoverable_authentication(
					&credential,
					discoverable_state,
					creds.as_slice(),
				)
				.map_err(|_| {
					ZauthError::LoginError(LoginError::PasskeyDiscoverableError)
				})
				.map(|result| (result, user))
		},
		Some(Either::Right((state, userid))) => {
			let credential = auth
				.credential
				.ok_or(ZauthError::LoginError(LoginError::PasskeyError))?;
			let username = auth.username.clone().ok_or(
				ZauthError::Unprocessable("username is missing".to_string()),
			)?;
			let user = User::find_by_username(username, db).await?;
			if userid != user.id {
				return Err(ZauthError::LoginError(LoginError::PasskeyError));
			}
			webauthn_store
				.webauthn
				.finish_passkey_authentication(&credential, &state)
				.map_err(|_| ZauthError::LoginError(LoginError::PasskeyError))
				.map(|result| (result, user))
		},
		None => Err(ZauthError::LoginError(LoginError::PasskeyError)),
	}?;

	let mut passkey = PassKey::find_by_cred_id(result.cred_id(), db).await?;

	passkey.set_last_used();

	// Update the stored counter
	let mut credential = passkey.credential()?;
	if result.needs_update()
		&& credential.update_credential(&result).is_some_and(|b| b)
	{
		passkey.set_credential(credential)?;
	}

	passkey.update(db).await?;

	Ok(user)
}

#[post("/webauthn/finish_auth", format = "json", data = "<auth>")]
pub async fn finish_authentication<'r>(
	webauthn_store: &State<WebAuthnStore>,
	auth: Json<PassKeyAuthentication>,
	cookies: &'r CookieJar<'_>,
	config: &'r State<Config>,
	db: DbConn,
) -> Result<Either<Redirect, impl Responder<'r, 'static>>> {
	match authenticate(webauthn_store, auth.into_inner(), &db).await {
		Ok(user) => {
			let session =
				Session::create(&user, config.user_session_duration(), &db)
					.await?;
			SessionCookie::new(session).login(cookies);
			user.update_last_login(&db).await?;
			Ok(Either::Left(stored_redirect_or(cookies, uri!(home_page))))
		},
		Err(ZauthError::LoginError(login_error)) => {
			Ok(Either::Right(template! {
				"session/login.html";
				error: Option<String> = Some(login_error.to_string()),
			}))
		},
		Err(e) => Err(e),
	}
}

#[get("/passkeys")]
pub async fn list_passkeys<'r>(
	db: DbConn,
	session: UserSession,
) -> Result<impl Responder<'r, 'static>> {
	let passkeys = PassKey::find_by_user_id(session.user.id, &db).await?;
	Ok(Accepter {
		html: template! {
			"passkeys/index.html";
			passkeys: Vec<PassKey> = passkeys.clone(),
			current_user: User = session.user
		},
		json: Json(passkeys),
	})
}

#[get("/passkeys/new")]
pub async fn new_passkey<'r>(
	session: UserSession,
) -> Result<impl Responder<'r, 'static>> {
	Ok(template! { "passkeys/new_passkey.html";
		current_user: User = session.user,
		errors: Option<String> = None,
	})
}

#[delete("/passkeys/<id>")]
pub async fn delete_passkey<'r>(
	id: i32,
	session: UserSession,
	db: DbConn,
) -> Result<impl Responder<'r, 'static>> {
	let passkey = PassKey::find(id, &db).await?;
	if session.user.id == passkey.user_id {
		passkey.delete(&db).await?;
		Ok(Accepter {
			html: Redirect::to(uri!(list_passkeys)),
			json: Custom(Status::NoContent, ()),
		})
	} else {
		Err(ZauthError::AuthError(AuthenticationError::Unauthorized(
			String::from("passkey is owned by another user"),
		)))
	}
}
