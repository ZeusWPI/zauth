extern crate base64;
extern crate chrono;
extern crate regex;
extern crate serde_urlencoded;

use chrono::{DateTime, Duration, Local};
use rocket::http::{Cookie, Cookies, Status};
use rocket::request::Form;
use rocket::response::status::{BadRequest, Custom};
use rocket::response::Redirect;
use rocket::State;
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;

use super::super::DbConn;
use models::client::*;
use models::user::*;

use rocket_http_authentication::BasicAuthentication;
use token_store::TokenStore;

pub const SESSION_VALIDITY_MINUTES: i64 = 60;

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct AuthorizationRequest {
	pub response_type: String,
	pub client_id:     String,
	pub redirect_uri:  String,
	pub scope:         Option<String>,
	pub state:         Option<String>,
}

#[derive(Serialize, Deserialize, Debug, FromForm, UriDisplayQuery)]
pub struct AuthState {
	pub client_id:    String,
	pub redirect_uri: String,
	pub scope:        Option<String>,
	pub client_state: Option<String>,
}

pub struct UserToken {
	pub user_id:      i32,
	pub username:     String,
	pub client_id:    String,
	pub redirect_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
	user_id: i32,
	expiry:  DateTime<Local>,
}

impl Session {
	pub fn new(user: User) -> Session {
		Session {
			user_id: user.id,
			expiry:  Local::now() + Duration::minutes(SESSION_VALIDITY_MINUTES),
		}
	}

	pub fn add_to_cookies(user: User, cookies: &mut Cookies) {
		let session = Session::new(user);
		let session_str = serde_urlencoded::to_string(session).unwrap();
		let session_cookie = Cookie::new("session", session_str);
		cookies.add_private(session_cookie);
	}

	pub fn user_from_cookies(
		cookies: &mut Cookies,
		conn: &DbConn,
	) -> Option<User>
	{
		cookies
			.get_private("session")
			.and_then(|cookie| serde_urlencoded::from_str(cookie.value()).ok())
			.and_then(|session: Self| session.user(conn))
	}

	fn user(&self, conn: &DbConn) -> Option<User> {
		if Local::now() > self.expiry {
			None
		} else {
			User::find(*&self.user_id, conn)
		}
	}
}

impl AuthState {
	pub fn redirect_uri_with_state(&self) -> String {
		let state_param =
			self.client_state.as_ref().map_or(String::new(), |s| {
				format!("state={}", urlencoding::encode(s))
			});
		format!("{}?{}", self.redirect_uri, state_param)
	}

	pub fn from_req(auth_req: AuthorizationRequest) -> AuthState {
		AuthState {
			client_id:    auth_req.client_id,
			redirect_uri: auth_req.redirect_uri,
			scope:        auth_req.scope,
			client_state: auth_req.state,
		}
	}

	pub fn encode_url(&self) -> String {
		serde_urlencoded::to_string(self).unwrap()
	}

	pub fn encode_b64(&self) -> String {
		base64::encode(&bincode::serialize(self).unwrap())
	}

	pub fn decode_b64(state_str: &str) -> Option<AuthState> {
		bincode::deserialize(&base64::decode(state_str).ok().unwrap()).ok()
	}
}

#[derive(Serialize)]
pub struct TemplateContext {
	client_name: String,
	state:       String,
}

impl TemplateContext {
	pub fn from_state(state: AuthState) -> TemplateContext {
		TemplateContext {
			client_name: state.client_id.clone(),
			state:       state.encode_b64(),
		}
	}
}

#[get("/oauth/authorize?<req..>")]
pub fn authorize(
	req: Form<AuthorizationRequest>,
) -> Result<Redirect, Custom<String>> {
	let req = req.into_inner();
	if !req.response_type.eq("code") {
		return Err(Custom(
			Status::NotImplemented,
			String::from("we only support authorization code"),
		));
	}
	if let Some(client) = Client::find(&req.client_id) {
		if client.redirect_uri_acceptable(&req.redirect_uri) {
			let state = AuthState::from_req(req);
			Ok(Redirect::to(uri!(login_get: state)))
		} else {
			Err(Custom(
				Status::Unauthorized,
				format!(
					"Redirect uri '{:?}' is not allowed for client with id \
					 '{}'",
					req.redirect_uri, req.client_id
				),
			))
		}
	} else {
		Err(Custom(
			Status::Unauthorized,
			format!(
				"Client with id '{}' is not known to this server",
				req.client_id
			),
		))
	}
}

#[get("/oauth/authorize")]
pub fn authorize_parse_failed() -> BadRequest<&'static str> {
	let msg = r#"
    The authorization request could not be processed,
    there are probably some parameters missing.
    "#;
	BadRequest(Some(msg))
}

#[derive(FromForm, Debug)]
pub struct LoginFormData {
	username:    String,
	password:    String,
	remember_me: bool,
	state:       String,
}

#[get("/oauth/login?<state..>")]
pub fn login_get(state: Form<AuthState>) -> Template {
	Template::render("login", TemplateContext::from_state(state.into_inner()))
}

#[get("/oauth/login")]
pub fn login_parse_failed() -> BadRequest<&'static str> {
	let msg = r#"
    The login request could not be processed,
    there are probably some parameters missing.
    "#;
	BadRequest(Some(msg))
}

#[post("/oauth/login", data = "<form>")]
pub fn login_post(
	mut cookies: Cookies,
	form: Form<LoginFormData>,
	conn: DbConn,
) -> Result<Redirect, Template>
{
	let data = form.into_inner();
	let state = AuthState::decode_b64(&data.state).unwrap();
	let user =
		User::find_and_authenticate(&data.username, &data.password, &conn);
	if let Some(user) = user {
		Session::add_to_cookies(user, &mut cookies);
		Ok(Redirect::to(uri!(grant_get: state)))
	} else {
		Err(Template::render(
			"login",
			TemplateContext::from_state(state),
		))
	}
}

#[derive(FromForm, Debug)]
pub struct GrantFormData {
	state: String,
	grant: bool,
}

#[derive(Responder)]
pub enum GrantResponse {
	T(Template),
	R(Redirect),
}

#[get("/oauth/grant?<state..>")]
pub fn grant_get<'a>(
	mut cookies: Cookies,
	state: Form<AuthState>,
	token_store: State<TokenStore<UserToken>>,
	conn: DbConn,
) -> Result<GrantResponse, Custom<String>>
{
	let user = Session::user_from_cookies(&mut cookies, &conn)
		.ok_or(Custom(Status::Unauthorized, String::from("No cookie :(")))?;

	if let Some(client) = Client::find(&state.client_id) {
		if client.needs_grant() {
			Ok(GrantResponse::T(Template::render(
				"grant",
				TemplateContext::from_state(state.into_inner()),
			)))
		} else {
			Ok(GrantResponse::R(authorization_granted(
				state.into_inner(),
				user,
				token_store.inner(),
			)))
		}
	} else {
		return Err(Custom(Status::NotFound, String::from("client not found")));
	}
}

#[post("/oauth/grant", data = "<form>")]
pub fn grant_post(
	mut cookies: Cookies,
	form: Form<GrantFormData>,
	token_store: State<TokenStore<UserToken>>,
	conn: DbConn,
) -> Result<Redirect, Custom<&'static str>>
{
	let user = Session::user_from_cookies(&mut cookies, &conn)
		.ok_or(Custom(Status::Unauthorized, "No cookie :("))?;
	let data = form.into_inner();
	let state = AuthState::decode_b64(&data.state).unwrap();
	if data.grant {
		Ok(authorization_granted(state, user, token_store.inner()))
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
		redirect_uri: state.redirect_uri.clone(),
	});
	Redirect::to(format!(
		"{}&code={}",
		state.redirect_uri_with_state(),
		authorization_code
	))
}

fn authorization_denied(state: AuthState) -> Redirect {
	Redirect::to(format!(
		"{}&error=access_denied",
		state.redirect_uri_with_state()
	))
}

#[derive(Serialize, Debug)]
pub struct TokenError {
	error:             String,
	error_description: Option<String>,
}

impl TokenError {
	fn json(msg: &str) -> Json<TokenError> {
		Json(TokenError {
			error:             String::from(msg),
			error_description: None,
		})
	}

	fn json_extra(msg: &str, extra: &str) -> Json<TokenError> {
		Json(TokenError {
			error:             String::from(msg),
			error_description: Some(String::from(extra)),
		})
	}
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
) -> Result<Json<TokenSuccess>, Json<TokenError>>
{
	let data = form.into_inner();
	let token = data.code.clone();
	let token_store = token_state.inner();

	let client = auth
		.map(|auth| (auth.username, auth.password))
		.or_else(|| Some((data.client_id?, data.client_secret?)))
		.and_then(|auth| Client::find_and_authenticate(&auth.0, &auth.1))
		.ok_or(TokenError::json("unauthorized_client"))?;

	let token = token_store
		.fetch_token(token)
		.ok_or(TokenError::json_extra("invalid_grant", "incorrect token"))?
		.item;

	if client.id == token.client_id {
		let access_token = token.username;
		Ok(TokenSuccess::json(access_token))
	} else {
		Err(TokenError::json_extra(
			"invalid grant",
			"token was not authorized to this client",
		))
	}
}
