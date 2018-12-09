extern crate serde_urlencoded;
extern crate chrono;
extern crate regex;
extern crate base64;

use rocket::Rocket;
use rocket::State;
use rocket::http::Cookies;
use rocket::response::status::{BadRequest,NotFound};
use rocket::response::Redirect;
use rocket::request::Form;
use rocket_contrib::templates::Template;
use rocket_contrib::json::Json;

use models::*;
use token_store::TokenStore;
use http_authentication::{BasicAuthentication};

pub const SESSION_VALIDITY_MINUTES : i64 = 60;

type MountPoint = &'static str;

pub fn mount<C : 'static + ClientProvider, U : 'static + UserProvider>(loc : &'static str, rocket : Rocket, client_provider : C, user_provider : U)
    -> Rocket {
        let mount_point : MountPoint = loc;
    rocket.mount(loc, routes![authorize, authorize_parse_failed, login_get, login_post, grant_get, grant_post, token])
        .manage(client_provider)
        .manage(user_provider)
        .manage(mount_point)
        .manage(TokenStore::new())
        .attach(Template::fairing())
}

pub trait ClientProvider : Sync + Send {
    fn client_exists(&self, client_id : &str) -> bool;
    fn client_has_uri(&self, client_id : &str,  redirect_uri : &str) -> bool;
    fn authorize_client(&self, client_id : &str, client_password : &str) -> bool;
}

pub trait UserProvider : Sync + Send {
    fn authorize_user(&self, user_id : &str, user_password : &str) -> bool;
    fn user_access_token(&self, user_id : &str) -> String;
}

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>
}

#[get("/authorize?<req..>")]
pub fn authorize(req: Form<AuthorizationRequest>, mp: State<MountPoint>)
    -> Result<Redirect, NotFound<String>> {
    let req = req.into_inner();
    if !req.response_type.eq("code"){
        return Err(NotFound(String::from("we only support authorization code")));
    }
    if let Some(client) = Client::find(&req.client_id) {
        if client.redirect_uri_acceptable(&req.redirect_uri) {
            let state = AuthState::from_req(req);
            Ok(Redirect::to(format!("{}{}", mp.inner(), uri!(login_get: state))))
        } else {
            Err(NotFound(format!("Redirect uri '{:?}' is not allowed for client with id '{}'", req.redirect_uri, client.id())))
        }
    } else {
        Err(NotFound(format!("Client with id '{}' is not known to this server", req.client_id)))
    }
}

#[get("/authorize")]
pub fn authorize_parse_failed() -> BadRequest<&'static str> {
    BadRequest(Some("The authorization request could not be parsed"))
}

#[derive(FromForm, Debug)]
struct LoginFormData {
    username: String,
    password: String,
    remember_me: bool,
    state: String
}

#[get("/login?<state..>")]
fn login_get(state : Form<AuthState>) -> Template {
    Template::render("login", TemplateContext::from_state(state.into_inner()))
}

#[post("/login", data="<form>")]
fn login_post(mut cookies : Cookies, form : Form<LoginFormData>, mp: State<MountPoint>) -> Result<Redirect, Template> {
    let data = form.into_inner();
    let state = AuthState::decode_b64(&data.state).unwrap();
    if let Some(user) = User::find_and_authenticate(&data.username, &data.password) {
        Session::add_to_cookies(&user, &mut cookies);
        Ok(Redirect::to(format!("{}{}", mp.inner(), uri!(grant_get: state))))
    } else {
        Err(Template::render("login", TemplateContext::from_state(state)))
    }
}

#[derive(FromForm, Debug)]
struct GrantFormData {
    state : String,
    grant : bool
}

#[get("/grant?<state..>")]
fn grant_get(mut cookies : Cookies, state : Form<AuthState>) -> Result<Template, String> {
    if let Some(_) = Session::from_cookies(&mut cookies) {
        Ok(Template::render("grant", TemplateContext::from_state(state.into_inner())))
    } else {
        Err(String::from("No cookie :("))
    }
}

#[post("/grant", data="<form>")]
fn grant_post(mut cookies : Cookies, form : Form<GrantFormData>, token_state : State<TokenStore>)
    -> Result<Redirect, String> {
    if let Some(session) = Session::from_cookies(&mut cookies) {
        let data = form.into_inner();
        let state = AuthState::decode_b64(&data.state).unwrap();
        if data.grant {
            Ok(authorization_granted(state, session.user(), token_state.inner()))
        } else {
            Ok(authorization_denied(state))
        }
    } else {
        Err(String::from("No cookie :("))
    }
}

fn authorization_granted(state : AuthState, user : User, token_store : &TokenStore) -> Redirect {
    let authorization_code = token_store.create_token(&state.client_id, &user, &state.redirect_uri);
    Redirect::to(format!("{}&code={}", state.redirect_uri_with_state(), authorization_code))
}

fn authorization_denied(state : AuthState) -> Redirect {
    Redirect::to(format!("{}&error=access_denied", state.redirect_uri_with_state()))
}

#[derive(FromForm, Debug)]
struct TokenFormData {
    grant_type : String,
    code : String,
    redirect_uri : String,
}

fn check_client_authentication(auth : BasicAuthentication) -> Option<Client> {
    Client::find(&auth.username)
}

#[derive(Serialize, Debug)]
struct TokenError {
    error : String,
    error_description : Option<String>
}

impl TokenError {
    fn json(msg : &str) -> Json<TokenError> {
        Json(TokenError {
            error : String::from(msg),
            error_description : None
        })
    }
    fn json_extra(msg : &str, extra : &str) -> Json<TokenError> {
        Json(TokenError {
            error : String::from(msg),
            error_description : Some(String::from(extra))
        })
    }
}

#[derive(Serialize, Debug)]
struct TokenSuccess {
    access_code : String,
    token_type : String,
    expires_in : u64,
}

impl TokenSuccess {
    fn json(username : String) -> Json<TokenSuccess> {
        Json(TokenSuccess {
            access_code : username.clone(),
            token_type : String::from("???"),
            expires_in: 1,
        })
    }
}



#[post("/token", data="<form>")]
fn token(auth : BasicAuthentication, form : Form<TokenFormData>, token_state : State<TokenStore>)
    -> Result<Json<TokenSuccess>, Json<TokenError>> {
    let data = form.into_inner();
    let token_store = token_state.inner();
    let client_opt = check_client_authentication(auth);
    if let Some(client) = client_opt {
        match token_store.fetch_token_username(&client, data.redirect_uri, data.code) {
            Ok(username) => Ok(TokenSuccess::json(username)),
            Err(msg)     => Err(TokenError::json_extra("invalid_grant", msg))
        }
    } else {
        // return 401, with WWW-Autheticate
        Err(TokenError::json("invalid_client"))
    }
}



#[cfg(test)]
mod test {
    extern crate rocket;
    extern crate urlencoding;
    extern crate serde_json;

    use super::*;
    use rocket::http::Status;
    use rocket::http::Header;
    use rocket::local::Client;
    use rocket::http::ContentType;
    use rocket::http::Cookie;
    use self::serde_json::Value;
    use regex::Regex;

    struct UserProviderImpl {}

    impl UserProvider for UserProviderImpl {
        fn authorize_user(&self, user_id : &str, user_password : &str) -> bool {
            true
        }
        fn user_access_token(&self, user_id : &str) -> String {
            format!("This is an access token for {}", user_id)
        }
    }

    struct ClientProviderImpl {}

    impl ClientProvider for ClientProviderImpl {
        fn client_exists(&self, client_id : &str) -> bool {
            true
        }
        fn client_has_uri(&self, client_id : &str,  redirect_uri : &str) -> bool {
            true
        }
        fn authorize_client(&self, client_id : &str, client_password : &str) -> bool {
            true
        }
    }

    fn create_http_client() -> Client {
        let cp = ClientProviderImpl {};
        let up = UserProviderImpl {};
        Client::new(mount("/oauth", rocket::ignite(), cp, up)).expect("valid rocket instance")
    }

    fn url(content : &str) -> String {
        urlencoding::encode(content)
    }

    fn get_param(param_name : &str, query : &String) -> Option<String> {
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

        // 1. User is redirected to OAuth server with request params given by the client
        //    The OAuth server should respond with a redirect to the login page.
        let authorize_url = format!(
            "/oauth/authorize?response_type=code&redirect_uri={}&client_id={}&state={}",
            url(redirect_uri),
            url(client_id),
            url(client_state)
            );
        let response = http_client.get(authorize_url).dispatch();

        assert_eq!(response.status(), Status::SeeOther);
        let login_location = response.headers().get_one("Location").expect("Location header");
        println!("login location: {}", login_location);
        assert!(login_location.starts_with("/oauth/login"));

        // 2. User requests the login page
        let mut response = http_client.get(login_location).dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::HTML));

        let state_regex = Regex::new("<input type=\"hidden\" name=\"state\" value=\"([^\"]+)\">").unwrap();
        let body = response.body_string().expect("response body");
        let form_state = state_regex.captures(&body).map(|c| c[1].to_string()).expect("hidden state field");

        // 3. User posts it credentials to the login path
        let login_url = "/oauth/login";
        let form_body = format!("username={}&password={}&state={}&remember_me=on",
                                url(user_username),
                                url(user_password),
                                form_state
                               );

        let response = http_client.post(login_url)
            .body(form_body)
            .header(ContentType::Form)
            .dispatch();

        assert_eq!(response.status(), Status::SeeOther);
        let grant_location = response.headers().get_one("Location").expect("Location header");
        assert!(grant_location.starts_with("/oauth/grant"));
        let session_cookie_str = response.headers().get_one("Set-Cookie").expect("Session cookie").to_owned();
        let cookie_regex = Regex::new("^([^=]+)=([^;]+).*").unwrap();
        let (cookie_name, cookie_content) = cookie_regex.captures(&session_cookie_str)
            .map(|c| (c[1].to_string(), urlencoding::decode(&c[2]).unwrap()))
            .expect("session cookie");

        // 4. User requests grant page
        let mut response = http_client.get(grant_location)
            .cookie(Cookie::new(cookie_name.to_string(), cookie_content.to_string()))
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::HTML));

        let state_regex = Regex::new("<input type=\"hidden\" name=\"state\" value=\"([^\"]+)\">").unwrap();
        let body = response.body_string().expect("response body");
        let form_state = state_regex.captures(&body)
            .map(|c| c[1].to_string())
            .expect("hidden state field");

        // 5. User posts to grant page
        let grant_url = "/oauth/grant";
        let form_body = format!("state={}&grant=true", form_state);

        let response = http_client.post(grant_url)
            .body(form_body)
            .cookie(Cookie::new(cookie_name.to_string(), cookie_content.to_string()))
            .header(ContentType::Form)
            .dispatch();

        assert_eq!(response.status(), Status::SeeOther);
        let redirect_location = response.headers().get_one("Location").expect("Location header");

        let redirect_uri_regex = Regex::new("^([^?]+)?(.*)$").unwrap();
        let (redirect_uri_base, redirect_uri_params) = redirect_uri_regex.captures(&redirect_location)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .unwrap();

        assert_eq!(redirect_uri_base, redirect_uri);

        let authorization_code = get_param("code", &redirect_uri_params).expect("authorization code");
        let state = get_param("state", &redirect_uri_params).expect("state");

        assert_eq!(client_state, urlencoding::decode(&state).expect("state decoded"));

        // 6. Client requests access code
        let token_url = "/oauth/token";
        let form_body = format!("grant_type=authorization_code&code={}&redirect_uri={}", authorization_code, redirect_uri);

        let credentials = base64::encode(&format!("{}:{}", client_id, client_secret));

        println!("Request body: {:?}", form_body);
        let req = http_client.post(token_url)
                                .header(ContentType::Form)
                                .header(Header::new("Authorization", format!("Basic {}", credentials)))
                                .body(form_body);

        let mut response = req.dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type().expect("content type"), ContentType::JSON);

        let response_body = response.body_string().expect("response body");
        println!("{}", response_body);
        let data : Value = serde_json::from_str(&response_body)
                            .expect("response json values");

        assert!(data["access_code"].is_string());
        assert!(data["token_type"].is_string());
        assert_eq!(data["token_type"], "???");
    }
}

