#![feature(async_closure)]

extern crate diesel;
extern crate regex;
extern crate rocket;
extern crate serde_json;
extern crate urlencoding;
extern crate zauth;

use self::serde_json::Value;
use regex::Regex;
use rocket::http::ContentType;
use rocket::http::Header;
use rocket::http::Status;

use zauth::controllers::oauth_controller::UserToken;
use zauth::models::client::{Client, NewClient};
use zauth::models::user::{NewUser, User};
use zauth::token_store::TokenStore;

mod common;
use crate::common::url;

fn get_param(param_name: &str, query: &String) -> Option<String> {
	Regex::new(&format!("{}=([^&]+)", param_name))
		.expect("valid regex")
		.captures(query)
		.map(|c| c[1].to_string())
}

#[rocket::async_test]
async fn normal_flow() {
	common::as_visitor(async move |http_client, db| {
		let redirect_uri = "https://example.com/redirect/me/here";
		let client_id = "test";
		let client_state = "anarchy (╯°□°)╯ ┻━┻";
		let user_username = "batman";
		let user_password = "wolololo";

		let user = User::create(
			NewUser {
				username:    String::from(user_username),
				password:    String::from(user_password),
				full_name:   String::from("abc"),
				email:       String::from("ghi@jkl.mno"),
				ssh_key:     Some(String::from("ssh-rsa pqrstuvwxyz")),
				not_a_robot: true,
			},
			common::BCRYPT_COST,
			&db,
		)
		.await
		.expect("user");

		let mut client = Client::create(
			NewClient {
				name: String::from(client_id),
			},
			&db,
		)
		.await
		.expect("client created");

		client.needs_grant = true;
		client.redirect_uri_list = String::from(redirect_uri);
		let client = client.update(&db).await.expect("client updated");

		// 1. User is redirected to OAuth server with request params given by
		// the client
		// The OAuth server should respond with a redirect the login page.
		let authorize_url = format!(
			"/oauth/authorize?response_type=code&redirect_uri={}&client_id={}&\
			 state={}",
			url(redirect_uri),
			url(client_id),
			url(client_state)
		);
		let response = http_client.get(authorize_url).dispatch().await;

		assert_eq!(response.status(), Status::SeeOther);
		let login_location = response
			.headers()
			.get_one("Location")
			.expect("Location header");

		assert!(login_location.starts_with("/login"));

		// 2. User requests the login page
		let response = http_client.get(login_location).dispatch().await;

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(response.content_type(), Some(ContentType::HTML));

		// 3. User posts it credentials to the login path
		let login_url = "/login";
		let form_body = format!(
			"username={}&password={}",
			url(user_username),
			url(user_password),
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

		// 4. User requests grant page
		let response = http_client.get(grant_location).dispatch().await;

		assert_eq!(response.status(), Status::Ok);
		assert_eq!(response.content_type(), Some(ContentType::HTML));

		// 5. User posts to grant page
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

		assert_eq!(redirect_uri_base, redirect_uri);

		let authorization_code = get_param("code", &redirect_uri_params)
			.expect("authorization code");
		let state = get_param("state", &redirect_uri_params).expect("state");

		// The client state we've sent in the beginning should be included in
		// the redirect back to the OAuth client
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
			base64::encode(&format!("{}:{}", client_id, client.secret));

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

		let response_body =
			response.into_string().await.expect("response body");
		let data: Value =
			serde_json::from_str(&response_body).expect("response json values");

		dbg!(&data);
		assert!(data["access_token"].is_string());
		assert!(data["token_type"].is_string());
		assert_eq!(data["token_type"], "???");

		// 6b. Client requests access code while sending its credentials
		//     trough the form body.

		// First, re-create a token
		let token_store = http_client
			.rocket()
			.state::<TokenStore<UserToken>>()
			.expect("should have token store");

		let authorization_code = token_store.create_token(UserToken {
			user_id:      user.id,
			username:     user.username.clone(),
			client_id:    client.id,
			client_name:  client.name,
			redirect_uri: String::from(redirect_uri),
		});

		let token_url = "/oauth/token";
		let form_body = format!(
			"grant_type=authorization_code&code={}&redirect_uri={}&\
			 client_id={}&client_secret={}",
			authorization_code, redirect_uri, client_id, client.secret
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

		dbg!(&data);
		assert!(data["access_token"].is_string());
		assert!(data["token_type"].is_string());
		assert_eq!(data["token_type"], "???");
	})
	.await;
}
