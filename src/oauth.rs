extern crate base64;
extern crate chrono;
extern crate regex;
extern crate serde_urlencoded;

use rocket::http::{Cookies, Status};
use rocket::request::Form;
use rocket::response::status::{BadRequest, Custom};
use rocket::response::Redirect;
use rocket::{Rocket, State};
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;

use http_authentication::BasicAuthentication;
use models::*;
use std::boxed::Box;
use token_store::TokenStore;
use util::redirect_to_relative;

pub const SESSION_VALIDITY_MINUTES: i64 = 60;

pub type MountPoint = &'static str;

#[derive(Clone)]
struct OAuth<U: UserProvider, C: ClientProvider> {
	user_provider:   U,
	client_provider: C,
}

pub fn mount<C: 'static + ClientProvider, U: 'static + UserProvider>(
	loc: &'static str,
	rocket: Rocket,
	client_provider: C,
	user_provider: U,
) -> Rocket
{
	let mount_point: MountPoint = loc;
	rocket
		.mount(
			loc,
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
		.manage(Box::new(client_provider) as Box<ClientProvider>)
		.manage(Box::new(user_provider) as Box<UserProvider>)
		.manage(mount_point)
		.manage(TokenStore::new())
		.attach(Template::fairing())
}

pub trait ClientProvider: Sync + Send {
	fn client_exists(&self, client_id: &str) -> bool;
	fn client_has_uri(&self, client_id: &str, redirect_uri: &str) -> bool;
	fn client_needs_grant(&self, client_id: &str) -> bool;
	fn authorize_client(&self, client_id: &str, client_password: &str) -> bool;
}

pub trait UserProvider: Sync + Send {
	fn authorize_user(&self, user_id: &str, user_password: &str) -> bool;
	fn user_access_token(&self, user_id: &str) -> String;
}

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct AuthorizationRequest {
	pub response_type: String,
	pub client_id:     String,
	pub redirect_uri:  String,
	pub scope:         Option<String>,
	pub state:         Option<String>,
}

#[get("/authorize?<req..>")]
pub fn authorize(
	req: Form<AuthorizationRequest>,
	cp: State<Box<ClientProvider>>,
	mp: State<MountPoint>,
) -> Result<Redirect, Custom<String>>
{
	let req = req.into_inner();
	if !req.response_type.eq("code") {
		Err(Custom(
			Status::NotImplemented,
			String::from("we only support authorization code"),
		))
	} else if !cp.client_exists(&req.client_id) {
		Err(Custom(
			Status::Unauthorized,
			format!(
				"Client with id '{}' is not known to this server",
				req.client_id
			),
		))
	} else if !cp.client_has_uri(&req.client_id, &req.redirect_uri) {
		Err(Custom(
			Status::Unauthorized,
			format!(
				"Redirect uri '{:?}' is not allowed for client with id '{}'",
				req.redirect_uri, req.client_id
			),
		))
	} else {
		let state = AuthState::from_req(req);
		Ok(redirect_to_relative(uri!(login_get: state), mp.inner()))
	}
}

#[get("/authorize")]
pub fn authorize_parse_failed() -> BadRequest<&'static str> {
	let msg = r#"
    The authorization request could not be processed,
    there are probably some parameters missing.
    "#;
	BadRequest(Some(msg))
}

#[derive(FromForm, Debug)]
struct LoginFormData {
	username:    String,
	password:    String,
	remember_me: bool,
	state:       String,
}

#[get("/login?<state..>")]
fn login_get(state: Form<AuthState>) -> Template {
	Template::render("login", TemplateContext::from_state(state.into_inner()))
}

#[get("/login")]
pub fn login_parse_failed() -> BadRequest<&'static str> {
	let msg = r#"
    The login request could not be processed,
    there are probably some parameters missing.
    "#;
	BadRequest(Some(msg))
}

#[post("/login", data = "<form>")]
fn login_post(
	mut cookies: Cookies,
	form: Form<LoginFormData>,
	mp: State<MountPoint>,
	user_provider: State<Box<dyn UserProvider>>,
) -> Result<Redirect, Template>
{
	let data = form.into_inner();
	let state = AuthState::decode_b64(&data.state).unwrap();
	if user_provider.authorize_user(&data.username, &data.password) {
		Session::add_to_cookies(&data.username, &mut cookies);
		Ok(redirect_to_relative(uri!(grant_get: state), mp.inner()))
	} else {
		Err(Template::render(
			"login",
			TemplateContext::from_state(state),
		))
	}
}

#[derive(FromForm, Debug)]
struct GrantFormData {
	state: String,
	grant: bool,
}

#[derive(Responder)]
enum GrantResponse {
	T(Template),
	R(Redirect),
}

#[get("/grant?<state..>")]
fn grant_get<'a>(
	mut cookies: Cookies,
	state: Form<AuthState>,
	token_store: State<TokenStore>,
	client_provider: State<Box<ClientProvider>>,
) -> Result<GrantResponse, Custom<String>>
{
	let session = Session::from_cookies(&mut cookies)
		.ok_or(Custom(Status::Unauthorized, String::from("No cookie :(")))?;

	if client_provider.client_needs_grant(&state.client_id) {
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
}

#[post("/grant", data = "<form>")]
fn grant_post(
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
struct TokenError {
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
struct TokenSuccess {
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
struct TokenFormData {
	grant_type:    String,
	code:          String,
	redirect_uri:  Option<String>,
	client_id:     Option<String>,
	client_secret: Option<String>,
}

fn get_authorization(
	cp: &Box<ClientProvider>,
	basic_auth: Option<BasicAuthentication>,
	client_id: Option<String>,
	client_secret: Option<String>,
) -> Option<String>
{
	let (id, secret) = if let Some(creds) = basic_auth {
		(creds.username, creds.password)
	} else {
		(client_id?, client_secret?)
	};
	if cp.authorize_client(&id, &secret) {
		Some(id)
	} else {
		None
	}
}

#[post("/token", data = "<form>")]
fn token(
	auth: Option<BasicAuthentication>,
	form: Form<TokenFormData>,
	user_provider: State<Box<UserProvider>>,
	client_provider: State<Box<ClientProvider>>,
	token_state: State<TokenStore>,
) -> Result<Json<TokenSuccess>, Json<TokenError>>
{
	let data = form.into_inner();
	let token_store = token_state.inner();

	let client = get_authorization(
		client_provider.inner(),
		auth,
		data.client_id,
		data.client_secret,
	)
	.ok_or(TokenError::json("unauthorized_client"))?;

	let token = token_store
		.fetch_token(data.code)
		.ok_or(TokenError::json_extra("invalid_grant", "incorrect token"))?;

	if client == token.client_id {
		let access_token = user_provider.user_access_token(&token.username);
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

	struct UserProviderImpl {}

	impl UserProvider for UserProviderImpl {
		fn authorize_user(&self, user_id: &str, user_password: &str) -> bool {
			true
		}

		fn user_access_token(&self, user_id: &str) -> String {
			format!("This is an access token for {}", user_id)
		}
	}

	struct ClientProviderImpl {}

	impl ClientProvider for ClientProviderImpl {
		fn client_exists(&self, client_id: &str) -> bool {
			true
		}

		fn client_has_uri(&self, client_id: &str, redirect_uri: &str) -> bool {
			true
		}

		fn client_needs_grant(&self, client_id: &str) -> bool {
			true
		}

		fn authorize_client(
			&self,
			client_id: &str,
			client_password: &str,
		) -> bool
		{
			true
		}
	}

	fn create_http_client() -> Client {
		let cp = ClientProviderImpl {};
		let up = UserProviderImpl {};
		Client::new(mount("/oauth", rocket::ignite(), cp, up))
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

		assert!(data["access_token"].is_string());
		assert!(data["token_type"].is_string());
		assert_eq!(data["token_type"], "???");
	}
}
