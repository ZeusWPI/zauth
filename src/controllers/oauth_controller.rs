use rocket::http::{Cookie, Cookies};
use rocket::request::Form;
use rocket::response::{Redirect, Responder};
use rocket::State;
use rocket_contrib::json::Json;
use std::fmt::Debug;

use crate::ephemeral::session::UserSession;
use crate::errors::Either::{Left, Right};
use crate::errors::*;
use crate::models::client::*;
use crate::models::user::*;
use crate::DbConn;

use crate::ephemeral::session::ensure_logged_in_and_redirect;
use crate::errors::OAuthError::InvalidCookie;
use crate::http_authentication::BasicAuthentication;
use crate::token_store::TokenStore;

const OAUTH_COOKIE: &str = "ZAUTH_OAUTH";

#[derive(Serialize, Deserialize, Debug, FromForm, UriDisplayQuery)]
pub struct AuthState {
	pub client_id:    i32,
	pub client_name:  String,
	pub redirect_uri: String,
	pub scope:        Option<String>,
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

	pub fn from_cookies(mut cookies: Cookies) -> Result<Self> {
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
	) -> AuthState
	{
		AuthState {
			client_id:    client.id,
			client_name:  client.name,
			redirect_uri: auth_req.redirect_uri,
			scope:        auth_req.scope,
			client_state: auth_req.state,
		}
	}

	pub fn encode_url(&self) -> String {
		serde_urlencoded::to_string(self).unwrap()
	}

	pub fn encode_b64(&self) -> Result<String> {
		Ok(base64::encode_config(
			&bincode::serialize(self).map_err(InternalError::from)?,
			base64::URL_SAFE_NO_PAD,
		))
	}

	pub fn decode_b64(state_str: &str) -> Result<AuthState> {
		Ok(bincode::deserialize(
			&base64::decode_config(state_str, base64::URL_SAFE_NO_PAD)
				.map_err(InternalError::from)?,
		)
		.map_err(InternalError::from)?)
	}
}

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct AuthorizationRequest {
	pub response_type: String,
	pub client_id:     String,
	pub redirect_uri:  String,
	pub scope:         Option<String>,
	pub state:         Option<String>,
}

#[get("/oauth/authorize?<req..>")]
pub fn authorize(
	mut cookies: Cookies,
	req: Form<AuthorizationRequest>,
	conn: DbConn,
) -> Result<Redirect>
{
	let req = req.into_inner();
	if !req.response_type.eq("code") {
		// This was NotImplemented error, but it makes no sense for a authorise
		// function not to return an AuthResult
		return Err(ZauthError::from(RequestError::ResponseTypeMismatch));
	}

	if let Ok(client) = Client::find_by_name(&req.client_id, &conn) {
		if client.redirect_uri_acceptable(&req.redirect_uri) {
			let state = AuthState::from_req(client, req);
			cookies.add_private(state.into_cookie()?);
			Ok(ensure_logged_in_and_redirect(cookies, uri!(grant_get)))
		} else {
			Err(AuthenticationError::Unauthorized(format!(
				"client with id {} is not authorized to use redirect_uri '{}'",
				req.client_id, req.redirect_uri
			))
			.into())
		}
	} else {
		Err(AuthenticationError::Unauthorized(format!(
			"client with id {} is not authorized on this server",
			req.client_id
		))
		.into())
	}
}

#[derive(FromForm, Debug)]
pub struct GrantFormData {
	grant: bool,
}

pub struct UserToken {
	pub user_id:      i32,
	pub username:     String,
	pub client_id:    i32,
	pub client_name:  String,
	pub redirect_uri: String,
}

#[get("/oauth/grant")]
pub fn grant_get<'a>(
	session: UserSession,
	cookies: Cookies,
	token_store: State<TokenStore<UserToken>>,
	conn: DbConn,
) -> Result<Either<impl Responder<'static>, impl Responder<'static>>>
{
	let state = AuthState::from_cookies(cookies)?;
	if let Ok(client) = Client::find(state.client_id, &conn) {
		if client.needs_grant {
			Ok(Left(template! {
				"oauth/grant.html";
				client_name: String = state.client_name.clone(),
			}))
		} else {
			Ok(Right(authorization_granted(
				state,
				session.user,
				token_store.inner(),
			)))
		}
	} else {
		Err(ZauthError::not_found("client not found"))
	}
}

#[post("/oauth/grant", data = "<form>")]
pub fn grant_post(
	session: UserSession,
	cookies: Cookies,
	form: Form<GrantFormData>,
	token_store: State<TokenStore<UserToken>>,
) -> Result<Redirect>
{
	let data = form.into_inner();
	let state = AuthState::from_cookies(cookies)?;
	if data.grant {
		Ok(authorization_granted(
			state,
			session.user,
			token_store.inner(),
		))
	} else {
		Ok(authorization_denied(state))
	}
}

fn authorization_granted(
	state: AuthState,
	user: User,
	token_store: &TokenStore<UserToken>,
) -> Redirect
{
	let authorization_code = token_store.create_token(UserToken {
		user_id:      user.id,
		username:     user.username.clone(),
		client_id:    state.client_id.clone(),
		client_name:  state.client_name.clone(),
		redirect_uri: state.redirect_uri.clone(),
	});
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
	token_type:   String,
	expires_in:   u64,
}

impl TokenSuccess {
	fn json(username: String) -> Json<TokenSuccess> {
		Json(TokenSuccess {
			access_token: username.clone(),
			token_type:   String::from("???"),
			expires_in:   1,
		})
	}
}

#[derive(FromForm, Debug)]
pub struct TokenFormData {
	grant_type:    String,
	code:          String,
	redirect_uri:  Option<String>,
	client_id:     Option<String>,
	client_secret: Option<String>,
}

#[post("/oauth/token", data = "<form>")]
pub fn token(
	auth: Option<BasicAuthentication>,
	form: Form<TokenFormData>,
	token_state: State<TokenStore<UserToken>>,
	conn: DbConn,
) -> Result<Json<TokenSuccess>>
{
	let data = form.into_inner();
	let token = data.code.clone();
	let token_store = token_state.inner();

	let client = auth
		.map(|auth| (auth.user, auth.password))
		.or_else(|| Some((data.client_id?, data.client_secret?)))
		.ok_or(ZauthError::from(RequestError::InvalidRequest))
		.and_then(|auth| {
			Client::find_and_authenticate(&auth.0, &auth.1, &conn).map_err(
				|e| match e {
					ZauthError::AuthError(_) => ZauthError::AuthError(
						AuthenticationError::Unauthorized(auth.0.to_string()),
					),
					e => e,
				},
			)
		})?;

	let token = token_store
		.fetch_token(token)
		.ok_or(ZauthError::from(AuthenticationError::InvalidGrant(
			"incorrect token".to_string(),
		)))?
		.item;

	if client.id == token.client_id {
		let access_token = token.username;
		Ok(TokenSuccess::json(access_token))
	} else {
		Err(ZauthError::from(AuthenticationError::InvalidGrant(
			"token was not authorized to this client".to_string(),
		)))
	}
}
