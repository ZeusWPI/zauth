extern crate diesel;
extern crate regex;
extern crate rocket;
extern crate serde_json;
extern crate urlencoding;
extern crate zauth;

use self::serde_json::Value;
use common::HttpClient;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::Validation;
use regex::Regex;
use rocket::http::Header;
use rocket::http::Status;
use rocket::http::{Accept, ContentType};

use zauth::controllers::oauth_controller::UserToken;
use zauth::models::client::{Client, NewClient};
use zauth::models::user::{NewUser, User};
use zauth::token_store::TokenStore;
use zauth::DbConn;

mod common;
use crate::common::url;

const REDIRECT_URI: &str = "https://example.com/redirect/me/here";
const CLIENT_ID: &str = "test";
const CLIENT_STATE: &str = "anarchy (╯°□°)╯ ┻━┻";
const USER_USERNAME: &str = "batman";
const USER_PASSWORD: &str = "wolololo";
const USER_EMAIL: &str = "test@test.com";

fn get_param(param_name: &str, query: &String) -> Option<String> {
	Regex::new(&format!("{}=([^&]+)", param_name))
		.expect("valid regex")
		.captures(query)
		.map(|c| c[1].to_string())
}

async fn create_user(db: &DbConn) -> User {
	User::create(
		NewUser {
			username:    String::from(USER_USERNAME),
			password:    String::from(USER_PASSWORD),
			full_name:   String::from("abc"),
			email:       String::from(USER_EMAIL),
			ssh_key:     Some(String::from("ssh-rsa pqrstuvwxyz")),
			not_a_robot: true,
		},
		common::BCRYPT_COST,
		db,
	)
	.await
	.expect("user")
}

async fn create_client(db: &DbConn) -> Client {
	let mut client = Client::create(
		NewClient {
			name: String::from(CLIENT_ID),
		},
		&db,
	)
	.await
	.expect("client created");

	client.needs_grant = true;
	client.redirect_uri_list = String::from(REDIRECT_URI);
	client.update(db).await.expect("client updated")
}

// Test all the usual oauth requests until `access_token/id_token` is retrieved.
async fn get_token(
	authorize_url: String,
	http_client: &HttpClient,
	client: &Client,
	user: &User,
) -> Value {
	let response = http_client.get(authorize_url).dispatch().await;
	assert_eq!(response.status(), Status::Ok);

	// 2. User accepts authorization to client
	// Server should respond with login redirect.
	let response = http_client
		.post("/oauth/authorize")
		.body("authorized=true")
		.header(ContentType::Form)
		.dispatch()
		.await;
	let login_location = response
		.headers()
		.get_one("Location")
		.expect("Location header");

	assert!(login_location.starts_with("/login"));

	// 3. User requests the login page
	let response = http_client.get(login_location).dispatch().await;

	assert_eq!(response.status(), Status::Ok);
	assert_eq!(response.content_type(), Some(ContentType::HTML));

	// 4. User posts it credentials to the login path
	let login_url = "/login";
	let form_body = format!(
		"username={}&password={}",
		url(&user.username),
		url(USER_PASSWORD),
	);

	let response = http_client
		.post(login_url)
		.body(form_body)
		.header(ContentType::Form)
		.dispatch()
		.await;

	assert_eq!(response.status(), Status::SeeOther);
	let grant_location = response
		.headers()
		.get_one("Location")
		.expect("Location header");

	assert!(grant_location.starts_with("/oauth/grant"));

	// 5. User requests grant page
	let response = http_client.get(grant_location).dispatch().await;

	assert_eq!(response.status(), Status::Ok);
	assert_eq!(response.content_type(), Some(ContentType::HTML));

	// 6. User posts to grant page
	let grant_url = "/oauth/grant";
	let grant_form_body = String::from("grant=true");

	let response = http_client
		.post(grant_url)
		.body(grant_form_body.clone())
		.header(ContentType::Form)
		.dispatch()
		.await;

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

	assert_eq!(redirect_uri_base, REDIRECT_URI);

	let authorization_code =
		get_param("code", &redirect_uri_params).expect("authorization code");
	let state = get_param("state", &redirect_uri_params).expect("state");

	// The client state we've sent in the beginning should be included in
	// the redirect back to the OAuth client
	assert_eq!(
		CLIENT_STATE,
		urlencoding::decode(&state).expect("state decoded")
	);

	// Log out user so we don't have their cookies anymore
	let response = http_client.post("/logout").dispatch().await;

	assert_eq!(response.status(), Status::SeeOther);

	// 7a. Client requests access code while sending its credentials
	//     trough HTTP Auth.
	let token_url = "/oauth/token";
	let form_body = format!(
		"grant_type=authorization_code&code={}&redirect_uri={}",
		authorization_code, REDIRECT_URI
	);

	let credentials =
		base64::encode(&format!("{}:{}", CLIENT_ID, client.secret));

	let req = http_client
		.post(token_url)
		.header(ContentType::Form)
		.header(Header::new(
			"Authorization",
			format!("Basic {}", credentials),
		))
		.body(form_body);

	let response = req.dispatch().await;

	assert_eq!(response.status(), Status::Ok);
	assert_eq!(
		response.content_type().expect("content type"),
		ContentType::JSON
	);

	let response_body = response.into_string().await.expect("response body");
	serde_json::from_str(&response_body).expect("response json values")
}

#[rocket::async_test]
async fn normal_flow() {
	common::as_visitor(async move |http_client, db| {
		let user = create_user(&db).await;
		let client = create_client(&db).await;

		// 1. User is redirected to OAuth server with request params given by
		// the client
		// The OAuth server should respond with the authorize page
		let authorize_url = format!(
			"/oauth/authorize?response_type=code&redirect_uri={}&client_id={}&\
			 state={}",
			url(REDIRECT_URI),
			url(CLIENT_ID),
			url(CLIENT_STATE)
		);

		// Do all the requests until access_token is retrieved.
		let data = get_token(authorize_url, &http_client, &client, &user).await;

		dbg!(&data);
		assert!(data["access_token"].is_string());
		assert!(data["token_type"].is_string());
		assert_eq!(data.get("id_token"), None);
		assert_eq!(data["token_type"], "bearer");

		// 7b. Client requests access code while sending its credentials
		//     trough the form body.

		// First, re-create a token
		let token_store = http_client
			.rocket()
			.state::<TokenStore<UserToken>>()
			.expect("should have token store");

		let authorization_code = token_store
			.create_token(UserToken {
				scope:        None,
				user_id:      user.id,
				username:     user.username.clone(),
				client_id:    client.id,
				client_name:  client.name,
				redirect_uri: String::from(REDIRECT_URI),
			})
			.await;

		let token_url = "/oauth/token";
		let form_body = format!(
			"grant_type=authorization_code&code={}&redirect_uri={}&\
			 client_id={}&client_secret={}",
			authorization_code, REDIRECT_URI, CLIENT_ID, client.secret
		);

		let req = http_client
			.post(token_url)
			.header(ContentType::Form)
			.body(form_body);

		let response = req.dispatch().await;

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.content_type().expect("content type"),
			ContentType::JSON
		);

		let response_body =
			response.into_string().await.expect("response body");
		let data: Value =
			serde_json::from_str(&response_body).expect("response json values");

		assert!(data["access_token"].is_string());
		assert_eq!(data["token_type"], "bearer");
		let token = data["access_token"].as_str().expect("access token");

		let response = http_client
			.get("/current_user")
			.header(Accept::JSON)
			.header(Header::new("Authorization", format!("Bearer {}", token)))
			.dispatch()
			.await;

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.content_type().expect("content type"),
			ContentType::JSON
		);

		let response_body =
			response.into_string().await.expect("response body");
		let data: Value =
			serde_json::from_str(&response_body).expect("response json values");

		assert!(data["id"].is_number());
		assert_eq!(data["username"], USER_USERNAME);
	})
	.await;
}

#[rocket::async_test]
async fn openid_flow() {
	common::as_visitor(async move |http_client, db| {
		let user = create_user(&db).await;
		let client = create_client(&db).await;

		let authorize_url = format!(
			"/oauth/authorize?response_type=code&redirect_uri={}&client_id={}&\
			 state={}&scope=openid",
			url(REDIRECT_URI),
			url(CLIENT_ID),
			url(CLIENT_STATE)
		);

		let data = get_token(authorize_url, &http_client, &client, &user).await;

		assert!(data["access_token"].is_string());
		assert!(data["token_type"].is_string());
		assert_ne!(data.get("id_token"), None);
		assert_eq!(data["token_type"], "bearer");

		let url = "/oauth/jwks";
		let req = http_client.get(url);
		let response = req.dispatch().await;
		let response_body =
			response.into_string().await.expect("response body");
		let jwk_set: JwkSet =
			serde_json::from_str(&response_body).expect("response json values");
		assert_eq!(jwk_set.keys.len(), 1);

		let mut validation = Validation::new(jsonwebtoken::Algorithm::ES384);
		validation.set_audience(&[CLIENT_ID]);
		validation.set_issuer(&["http://localhost:8000"]);

		let id_token = jsonwebtoken::decode::<Value>(
			data["id_token"].as_str().unwrap(),
			&DecodingKey::from_jwk(&jwk_set.keys.get(0).unwrap()).unwrap(),
			&validation,
		)
		.expect("id token")
		.claims;
		assert_eq!(id_token["preferred_username"], USER_USERNAME);
		assert_eq!(id_token["email"], USER_EMAIL);
	})
	.await;
}
