use rocket::http::{Cookies, Status};
use rocket::request::Form;
use rocket::response::status::Custom;
use rocket::response::Redirect;
use rocket::State;
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;

use ephemeral::session::{Session, UserSession};
use models::client::*;
use models::user::*;
use DbConn;

use http_authentication::BasicAuthentication;
use token_store::TokenStore;

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

	pub fn encode_b64(&self) -> String {
		base64::encode_config(
			&bincode::serialize(self).unwrap(),
			base64::URL_SAFE_NO_PAD,
		)
	}

	pub fn decode_b64(state_str: &str) -> Option<AuthState> {
		bincode::deserialize(
			&base64::decode_config(state_str, base64::URL_SAFE_NO_PAD)
				.ok()
				.unwrap(),
		)
		.ok()
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
			client_name: state.client_name.clone(),
			state:       state.encode_b64(),
		}
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
	req: Form<AuthorizationRequest>,
	conn: DbConn,
) -> Result<Redirect, Custom<String>>
{
	let req = req.into_inner();
	if !req.response_type.eq("code") {
		return Err(Custom(
			Status::NotImplemented,
			String::from("only response_type=code is supported"),
		));
	}
	if let Some(client) = Client::find_by_name(&req.client_id, &conn) {
		if client.redirect_uri_acceptable(&req.redirect_uri) {
			let state = AuthState::from_req(client, req);
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

pub struct UserToken {
	pub user_id:      i32,
	pub username:     String,
	pub client_id:    i32,
	pub client_name:  String,
	pub redirect_uri: String,
}

#[get("/oauth/grant?<state..>")]
pub fn grant_get<'a>(
	session: UserSession,
	state: Form<AuthState>,
	token_store: State<TokenStore<UserToken>>,
	conn: DbConn,
) -> Result<GrantResponse, Custom<String>>
{
	if let Some(client) = Client::find(state.client_id, &conn) {
		if client.needs_grant {
			Ok(GrantResponse::T(Template::render(
				"oauth/grant",
				TemplateContext::from_state(state.into_inner()),
			)))
		} else {
			Ok(GrantResponse::R(authorization_granted(
				state.into_inner(),
				session.user,
				token_store.inner(),
			)))
		}
	} else {
		return Err(Custom(Status::NotFound, String::from("client not found")));
	}
}

#[post("/oauth/grant", data = "<form>")]
pub fn grant_post(
	session: UserSession,
	form: Form<GrantFormData>,
	token_store: State<TokenStore<UserToken>>,
) -> Result<Redirect, Custom<&'static str>>
{
	let data = form.into_inner();
	let state = AuthState::decode_b64(&data.state).unwrap();
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
	conn: DbConn,
) -> Result<Json<TokenSuccess>, Json<TokenError>>
{
	let data = form.into_inner();
	let token = data.code.clone();
	let token_store = token_state.inner();

	let client = auth
		.map(|auth| (auth.user, auth.password))
		.or_else(|| Some((data.client_id?, data.client_secret?)))
		.and_then(|auth| Client::find_and_authenticate(&auth.0, &auth.1, &conn))
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
