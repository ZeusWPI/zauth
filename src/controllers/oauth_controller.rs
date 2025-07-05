use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::jwk::JwkSet;
use rocket::State;
use rocket::form::Form;
use rocket::http::{Cookie, CookieJar};
use rocket::response::{Redirect, Responder};
use rocket::serde::json::Json;
use std::fmt::Debug;

use crate::DbConn;
use crate::config::Config;
use crate::ephemeral::session::UserSession;
use crate::errors::Either::{Left, Right};
use crate::errors::*;
use crate::http_authentication::BasicAuthentication;
use crate::jwt::JWTBuilder;
use crate::models::client::*;
use crate::models::session::*;
use crate::models::user::*;

use crate::ephemeral::session::ensure_logged_in_and_redirect;
use crate::errors::OAuthError::InvalidCookie;
use crate::token_store::TokenStore;

const OAUTH_COOKIE: &str = "ZAUTH_OAUTH";

#[derive(Serialize, Deserialize, Debug, FromForm, UriDisplayQuery)]
pub struct AuthState {
	pub client_id: i32,
	pub client_name: String,
	pub redirect_uri: String,
	pub scope: Option<String>,
	pub client_state: Option<String>,
}

impl AuthState {
	pub fn redirect_uri_with_state(&self) -> String {
		let state_param =
			self.client_state.as_ref().map_or(String::new(), |s| {
				format!("state={}", urlencoding::encode(s))
			});
		format!("{}?{}", self.redirect_uri, state_param)
	}

	pub fn from_cookies(cookies: &CookieJar) -> Result<Self> {
		cookies
			.get_private(OAUTH_COOKIE)
			.and_then(|cookie| Self::decode_b64(cookie.value()).ok())
			.ok_or(ZauthError::OAuth(InvalidCookie))
	}

	pub fn into_cookie(self) -> Result<Cookie<'static>> {
		Ok(Cookie::new(OAUTH_COOKIE, self.encode_b64()?))
	}

	pub fn from_req(
		client: Client,
		auth_req: AuthorizationRequest,
	) -> AuthState {
		AuthState {
			client_id: client.id,
			client_name: client.name,
			redirect_uri: auth_req.redirect_uri,
			scope: auth_req.scope,
			client_state: auth_req.state,
		}
	}

	pub fn encode_url(&self) -> String {
		serde_urlencoded::to_string(self).unwrap()
	}

	pub fn encode_b64(&self) -> Result<String> {
		Ok(URL_SAFE_NO_PAD.encode(
			&bincode::serde::encode_to_vec(self, bincode::config::legacy())
				.map_err(InternalError::from)?,
		))
	}

	pub fn decode_b64(state_str: &str) -> Result<AuthState> {
		Ok(bincode::serde::decode_from_slice(
			&URL_SAFE_NO_PAD
				.decode(state_str)
				.map_err(InternalError::from)?,
			bincode::config::legacy(),
		)
		.map_err(InternalError::from)?
		.0)
	}
}

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct AuthorizationRequest {
	pub response_type: String,
	pub client_id: String,
	pub redirect_uri: String,
	pub scope: Option<String>,
	pub state: Option<String>,
}

#[get("/oauth/authorize?<req..>")]
pub async fn authorize<'r>(
	cookies: &CookieJar<'_>,
	req: AuthorizationRequest,
	db: DbConn,
) -> Result<impl Responder<'r, 'static> + use<'r>> {
	if !req.response_type.eq("code") {
		// This was NotImplemented error, but it makes no sense for a authorise
		// function not to return an AuthResult
		return Err(ZauthError::from(OAuthError::ResponseTypeMismatch));
	}

	match Client::find_by_name(req.client_id.to_owned(), &db).await {
		Ok(client) => {
			if client.redirect_uri_acceptable(&req.redirect_uri) {
				let client_description = client.description.clone();
				let state = AuthState::from_req(client, req);
				cookies.add_private(state.into_cookie()?);
				Ok(template! {
					"oauth/authorize.html";
					authorize_post_url: String = uri!(do_authorize).to_string(),
					client_description: String = client_description,
				})
			} else {
				Err(AuthenticationError::Unauthorized(format!(
					"client with id {} is not authorized to use redirect_uri '{}'",
					req.client_id, req.redirect_uri
				))
				.into())
			}
		},
		_ => Err(AuthenticationError::Unauthorized(format!(
			"client with id {} is not authorized on this server",
			req.client_id
		))
		.into()),
	}
}

#[derive(FromForm, Debug)]
pub struct AuthorizeFormData {
	authorized: bool,
}

#[post("/oauth/authorize", data = "<form>")]
pub async fn do_authorize(
	cookies: &CookieJar<'_>,
	form: Form<AuthorizeFormData>,
) -> Result<Redirect> {
	let state = AuthState::from_cookies(cookies)?;
	if form.into_inner().authorized {
		Ok(ensure_logged_in_and_redirect(cookies, uri!(grant_get)))
	} else {
		Ok(authorization_denied(state))
	}
}

#[derive(FromForm, Debug)]
pub struct GrantFormData {
	grant: bool,
}

pub struct UserToken {
	pub user_id: i32,
	pub username: String,
	pub client_id: i32,
	pub client_name: String,
	pub redirect_uri: String,
	pub scope: Option<String>,
}

#[get("/oauth/grant")]
pub async fn grant_get<'r>(
	session: UserSession,
	cookies: &CookieJar<'_>,
	token_store: &State<TokenStore<UserToken>>,
	db: DbConn,
) -> Result<
	Either<
		impl Responder<'r, 'static> + use<'r>,
		impl Responder<'r, 'static> + use<'r>,
	>,
> {
	let state = AuthState::from_cookies(cookies)?;
	match Client::find(state.client_id, &db).await {
		Ok(client) => {
			if client.needs_grant {
				Ok(Left(template! {
					"oauth/grant.html";
					client_description: String = client.description.clone(),
				}))
			} else {
				Ok(Right(
					authorization_granted(
						state,
						session.user,
						token_store.inner(),
					)
					.await,
				))
			}
		},
		_ => Err(ZauthError::not_found("client not found")),
	}
}

#[post("/oauth/grant", data = "<form>")]
pub async fn grant_post<'r>(
	session: UserSession,
	cookies: &CookieJar<'_>,
	form: Form<GrantFormData>,
	token_store: &State<TokenStore<UserToken>>,
) -> Result<impl Responder<'r, 'static> + use<'r>> {
	let data = form.into_inner();
	let state = AuthState::from_cookies(cookies)?;
	if data.grant {
		Ok(
			authorization_granted(state, session.user, token_store.inner())
				.await,
		)
	} else {
		Ok(authorization_denied(state))
	}
}

async fn authorization_granted(
	state: AuthState,
	user: User,
	token_store: &TokenStore<UserToken>,
) -> Redirect {
	let authorization_code = token_store
		.create_token(UserToken {
			user_id: user.id,
			scope: state.scope.clone(),
			username: user.username.clone(),
			client_id: state.client_id.clone(),
			client_name: state.client_name.clone(),
			redirect_uri: state.redirect_uri.clone(),
		})
		.await;
	let uri = format!(
		"{}&code={}",
		state.redirect_uri_with_state(),
		authorization_code
	);
	Redirect::to(uri)
}

fn authorization_denied(state: AuthState) -> Redirect {
	Redirect::to(format!(
		"{}&error=access_denied",
		state.redirect_uri_with_state()
	))
}

#[derive(Serialize, Debug)]
pub struct TokenSuccess {
	access_token: String,
	token_type: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	id_token: Option<String>,
	expires_in: i64,
}

#[derive(FromForm, Debug)]
pub struct TokenFormData {
	grant_type: String,
	code: String,
	redirect_uri: String,
	client_id: Option<String>,
	client_secret: Option<String>,
}

#[post("/oauth/token", data = "<form>")]
pub async fn token(
	auth: Option<BasicAuthentication>,
	form: Form<TokenFormData>,
	config: &State<Config>,
	token_state: &State<TokenStore<UserToken>>,
	jwt_builder: &State<JWTBuilder>,
	db: DbConn,
) -> Result<Json<TokenSuccess>> {
	let data = form.into_inner();

	if !data.grant_type.eq("authorization_code") {
		return Err(ZauthError::from(OAuthError::GrantTypeMismatch));
	}

	let token = data.code.clone();
	let data_redirect_uri = data.redirect_uri.clone();
	let token_store = token_state.inner();

	let auth = auth
		.map(|auth| (auth.user, auth.password))
		.or_else(|| Some((data.client_id?, data.client_secret?)))
		.ok_or(ZauthError::from(OAuthError::InvalidRequest))?;

	let client =
		match Client::find_and_authenticate(auth.0.to_string(), &auth.1, &db)
			.await
		{
			Ok(client) => client,
			Err(ZauthError::AuthError(_)) => {
				return Err(ZauthError::AuthError(
					AuthenticationError::Unauthorized(auth.0),
				));
			},
			Err(e) => return Err(e),
		};

	let token = token_store
		.fetch_token(token)
		.await
		.ok_or(ZauthError::from(OAuthError::InvalidGrant(
			"incorrect token".to_string(),
		)))?
		.item;

	if client.id != token.client_id {
		Err(ZauthError::from(OAuthError::InvalidGrant(
			"token was not authorized to this client".to_string(),
		)))
	} else if token.redirect_uri != data_redirect_uri {
		Err(ZauthError::from(OAuthError::InvalidGrant(
			"redirect uri does not match".to_string(),
		)))
	} else {
		let user = User::find(token.user_id, &db).await?;
		let id_token = token
			.scope
			.as_ref()
			.map(|scope| -> Option<String> {
				match scope.contains("openid") {
					true => {
						jwt_builder.encode_id_token(&client, &user, config).ok()
					},
					false => None,
				}
			})
			.flatten();

		let session = Session::create_client_session(
			&user,
			&client,
			token.scope,
			&config,
			&db,
		)
		.await?;
		Ok(Json(TokenSuccess {
			access_token: session.key.unwrap().clone(),
			token_type: String::from("bearer"),
			id_token,
			expires_in: config.client_session_seconds,
		}))
	}
}

#[get("/oauth/jwks")]
pub async fn jwks(jwt_builder: &State<JWTBuilder>) -> Json<JwkSet> {
	Json(jwt_builder.jwks.clone())
}
