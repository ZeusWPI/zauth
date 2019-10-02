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

use models::client::*;
use models::user::*;

use rocket_http_authentication::BasicAuthentication;
use token_store::TokenStore;

pub const SESSION_VALIDITY_MINUTES: i64 = 60;

pub type MountPoint = &'static str;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
	username: String,
	expiry:   DateTime<Local>,
}

impl Session {
	pub fn new(username: String) -> Session {
		let expiry = Local::now() + Duration::minutes(SESSION_VALIDITY_MINUTES);
		Session { username, expiry }
	}

	pub fn user(&self) -> User {
		User::find(&self.username).expect("session for unexisting user")
	}

	pub fn add_to_cookies(username: &str, cookies: &mut Cookies) {
		let session = Session::new(String::from(username));
		let session_str = serde_urlencoded::to_string(session).unwrap();
		let session_cookie = Cookie::new("session", session_str);
		cookies.add_private(session_cookie);
	}

	pub fn from_cookies(cookies: &mut Cookies) -> Option<Session> {
		cookies
			.get_private("session")
			.and_then(|cookie| serde_urlencoded::from_str(cookie.value()).ok())
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
) -> Result<Redirect, Template>
{
	let data = form.into_inner();
	let state = AuthState::decode_b64(&data.state).unwrap();
	if let Some(user) =
		User::find_and_authenticate(&data.username, &data.password)
	{
		Session::add_to_cookies(&data.username, &mut cookies);
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
	token_store: State<TokenStore>,
) -> Result<GrantResponse, Custom<String>>
{
	let session = Session::from_cookies(&mut cookies)
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
				session.user(),
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
	token_store: State<TokenStore>,
) -> Result<Redirect, Custom<&'static str>>
{
	let session = Session::from_cookies(&mut cookies)
		.ok_or(Custom(Status::Unauthorized, "No cookie :("))?;
	let data = form.into_inner();
	let state = AuthState::decode_b64(&data.state).unwrap();
	if data.grant {
		Ok(authorization_granted(
			state,
			session.user(),
			token_store.inner(),
		))
	} else {
		Ok(authorization_denied(state))
	}
}

fn authorization_granted(
	state: AuthState,
	user: User,
	token_store: &TokenStore,
) -> Redirect
{
	let authorization_code =
		token_store.create_token(&state.client_id, &user, &state.redirect_uri);
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
	token_state: State<TokenStore>,
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
		.ok_or(TokenError::json_extra("invalid_grant", "incorrect token"))?;

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

#[cfg(test)]
mod test {
	extern crate rocket;
	extern crate serde_json;
	extern crate urlencoding;

	use self::serde_json::Value;
	use super::*;
	use regex::Regex;
	use rocket::http::ContentType;
	use rocket::http::Cookie;
	use rocket::http::Header;
	use rocket::http::Status;
	use rocket::local::Client;

	fn create_http_client() -> Client {
		Client::new(
			rocket::ignite()
				.mount(
					"/",
					routes![
						authorize,
						authorize_parse_failed,
						login_get,
						login_post,
						grant_get,
						grant_post,
						token
					],
				)
				.manage(TokenStore::new())
				.attach(Template::fairing()),
		)
		.expect("valid rocket instance")
	}

	fn url(content: &str) -> String {
		urlencoding::encode(content)
	}

	fn get_param(param_name: &str, query: &String) -> Option<String> {
		Regex::new(&format!("{}=([^&]+)", param_name))
			.expect("valid regex")
			.captures(query)
			.map(|c| c[1].to_string())
	}

	#[test]
	fn normal_flow() {
		let http_client = create_http_client();

		let redirect_uri = "https://example.com/redirect/me/here";
		let client_id = "test";
		let client_secret = "nananana";
		let client_state = "anarchy (╯°□°)╯ ┻━┻";
		let user_username = "batman";
		let user_password = "wolololo";

		// 1. User is redirected to OAuth server with request params given by
		// the client    The OAuth server should respond with a redirect to
		// the login page.
		let authorize_url = format!(
            "/oauth/authorize?response_type=code&redirect_uri={}&client_id={}&state={}",
            url(redirect_uri),
            url(client_id),
            url(client_state)
        );
		let response = http_client.get(authorize_url).dispatch();

		assert_eq!(response.status(), Status::SeeOther);
		let login_location = response
			.headers()
			.get_one("Location")
			.expect("Location header");
		dbg!(login_location);
		assert!(login_location.starts_with("/oauth/login"));

		// 2. User requests the login page
		let mut response = http_client.get(login_location).dispatch();

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(response.content_type(), Some(ContentType::HTML));

		let state_regex = Regex::new(
			"<input type=\"hidden\" name=\"state\" value=\"([^\"]+)\">",
		)
		.unwrap();
		let body = response.body_string().expect("response body");
		let form_state = state_regex
			.captures(&body)
			.map(|c| c[1].to_string())
			.expect("hidden state field");

		// 3. User posts it credentials to the login path
		let login_url = "/oauth/login";
		let form_body = format!(
			"username={}&password={}&state={}&remember_me=on",
			url(user_username),
			url(user_password),
			form_state
		);

		let response = http_client
			.post(login_url)
			.body(form_body)
			.header(ContentType::Form)
			.dispatch();

		assert_eq!(response.status(), Status::SeeOther);
		let grant_location = response
			.headers()
			.get_one("Location")
			.expect("Location header");
		assert!(grant_location.starts_with("/oauth/grant"));
		let session_cookie_str = response
			.headers()
			.get_one("Set-Cookie")
			.expect("Session cookie")
			.to_owned();
		let cookie_regex = Regex::new("^([^=]+)=([^;]+).*").unwrap();
		let (cookie_name, cookie_content) = cookie_regex
			.captures(&session_cookie_str)
			.map(|c| (c[1].to_string(), urlencoding::decode(&c[2]).unwrap()))
			.expect("session cookie");

		// 4. User requests grant page
		let mut response = http_client
			.get(grant_location)
			.cookie(Cookie::new(
				cookie_name.to_string(),
				cookie_content.to_string(),
			))
			.dispatch();

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(response.content_type(), Some(ContentType::HTML));

		let state_regex = Regex::new(
			"<input type=\"hidden\" name=\"state\" value=\"([^\"]+)\">",
		)
		.unwrap();
		let body = response.body_string().expect("response body");
		let form_state = state_regex
			.captures(&body)
			.map(|c| c[1].to_string())
			.expect("hidden state field");

		// 5. User posts to grant page
		let grant_url = "/oauth/grant";
		let grant_form_body = format!("state={}&grant=true", form_state);

		let response = http_client
			.post(grant_url)
			.body(grant_form_body.clone())
			.cookie(Cookie::new(
				cookie_name.to_string(),
				cookie_content.to_string(),
			))
			.header(ContentType::Form)
			.dispatch();

		assert_eq!(response.status(), Status::SeeOther);
		let redirect_location = response
			.headers()
			.get_one("Location")
			.expect("Location header");

		let redirect_uri_regex = Regex::new("^([^?]+)?(.*)$").unwrap();
		let (redirect_uri_base, redirect_uri_params) = redirect_uri_regex
			.captures(&redirect_location)
			.map(|c| (c[1].to_string(), c[2].to_string()))
			.unwrap();

		assert_eq!(redirect_uri_base, redirect_uri);

		let authorization_code = get_param("code", &redirect_uri_params)
			.expect("authorization code");
		let state = get_param("state", &redirect_uri_params).expect("state");

		assert_eq!(
			client_state,
			urlencoding::decode(&state).expect("state decoded")
		);

		// 6a. Client requests access code while sending its credentials
		//     trough HTTP Auth.
		let token_url = "/oauth/token";
		let form_body = format!(
			"grant_type=authorization_code&code={}&redirect_uri={}",
			authorization_code, redirect_uri
		);

		let credentials =
			base64::encode(&format!("{}:{}", client_id, client_secret));

		let req = http_client
			.post(token_url)
			.header(ContentType::Form)
			.header(Header::new(
				"Authorization",
				format!("Basic {}", credentials),
			))
			.body(form_body);

		let mut response = req.dispatch();

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.content_type().expect("content type"),
			ContentType::JSON
		);

		let response_body = response.body_string().expect("response body");
		let data: Value =
			serde_json::from_str(&response_body).expect("response json values");

		assert!(data["access_token"].is_string());
		assert!(data["token_type"].is_string());
		assert_eq!(data["token_type"], "???");

		// 6b. Client requests access code while sending its credentials
		//     trough the form body.

		// First, re-create a token
		let token_store = http_client
			.rocket()
			.state::<TokenStore>()
			.expect("should have token store");
		let user = &User::find(&String::from(user_username)).unwrap();
		let authorization_code = token_store.create_token(
			&String::from(client_id),
			user,
			&String::from(redirect_uri),
		);

		let token_url = "/oauth/token";
		let form_body = format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}",
            authorization_code, redirect_uri, client_id, client_secret
        );

		let req = http_client
			.post(token_url)
			.header(ContentType::Form)
			.body(form_body);

		let mut response = req.dispatch();

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.content_type().expect("content type"),
			ContentType::JSON
		);

		let response_body = response.body_string().expect("response body");
		let data: Value =
			serde_json::from_str(&response_body).expect("response json values");

		dbg!(&data);
		assert!(data["access_token"].is_string());
		assert!(data["token_type"].is_string());
		assert_eq!(data["token_type"], "???");
	}
}
