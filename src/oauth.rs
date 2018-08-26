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
use rocket_contrib::Template;
use rocket_contrib::Json;

use models::*;
use token_store::TokenStore;
use http_authentication::{BasicAuthentication};

pub const SESSION_VALIDITY_MINUTES : i64 = 60;

pub fn mount(loc : &'static str, rocket : Rocket) -> Rocket {
    rocket.mount(loc, routes![authorize, authorize_parse_failed, login_get, login_post, grant_get, grant_post, token])
        .manage(TokenStore::new())
        .attach(Template::fairing())
}

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>
}

#[get("/authorize?<req>")]
pub fn authorize(req : AuthorizationRequest) -> Result<Redirect, NotFound<String>> {
    if !req.response_type.eq("code"){
        return Err(NotFound(String::from("we only support authorization code")));
    }
    if let Some(client) = Client::find(&req.client_id) {
        if client.redirect_uri_acceptable(&req.redirect_uri) {
            let state = AuthState::from_req(req);
            Ok(Redirect::to(&format!("./login?{}", state.encode_url())))
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

#[get("/login?<state>")]
fn login_get(state : AuthState) -> Template {
    Template::render("login", TemplateContext::from_state(state))
}

#[post("/login", data="<form>")]
fn login_post(mut cookies : Cookies, form : Form<LoginFormData>) -> Result<Redirect, Template> {
    let data = form.into_inner();
    let state = AuthState::decode_b64(&data.state).unwrap();
    if let Some(user) = User::find_and_authenticate(&data.username, &data.password) {
        Session::add_to_cookies(&user, &mut cookies);
        Ok(Redirect::to(&format!("./grant?{}", state.encode_url())))
    } else {
        Err(Template::render("login", TemplateContext::from_state(state)))
    }
}

#[derive(FromForm, Debug)]
struct GrantFormData {
    state : String,
    grant : bool
}

#[get("/grant?<state>")]
fn grant_get(mut cookies : Cookies, state : AuthState) -> Result<Template, String> {
    println!("Cookies: {:?}", cookies);
    if let Some(_) = Session::from_cookies(&mut cookies) {
        Ok(Template::render("grant", TemplateContext::from_state(state)))
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
    Redirect::to(&format!("{}&code={}", state.redirect_uri_with_state(), authorization_code))
}

fn authorization_denied(state : AuthState) -> Redirect {
    Redirect::to(&format!("{}&error=access_denied", state.redirect_uri_with_state()))
}

#[derive(FromForm, Debug)]
struct TokenFormData {
    grant_type : String,
    code : String,
    redirect_uri : String,
    client_id : String
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
        if client.id() == &data.client_id {
            return match token_store.fetch_token_username(&client, data.redirect_uri, data.code) {
                Ok(username) => Ok(TokenSuccess::json(username)),
                Err(msg)     => Err(TokenError::json_extra("invalid_grant", msg))
            }
        } else {
            return Err(TokenError::json("invalid_grant"))
        }
    }
    // return 401, with WWW-Autheticate
    Err(TokenError::json("invalid_client"))
}



#[cfg(test)]
mod test {
    extern crate rocket;
    extern crate percent_encoding;

    use super::*;
    use rocket::http::Status;
    use rocket::local::Client;
    use rocket::http::ContentType;
    use rocket::http::Cookie;
    use regex::Regex;
    use self::percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};

    fn create_http_client() -> Client {
        Client::new(mount("/oauth", rocket::ignite())).expect("valid rocket instance")
    }

    fn url(content : &str) -> String {
        utf8_percent_encode(content, DEFAULT_ENCODE_SET).to_string()
    }

    #[test]
    fn normal_flow() {
        let http_client = create_http_client();

        let redirect_uri = "https://example.com/redirect/me/here";
        let client_id = "test";
        let client_secret = "nananana";
        let user_username = "batman";
        let user_password = "wolololo";
        let state = "anarchy (╯°□°)╯ ┻━┻";

        // 1. User is redirected to OAuth server with request params given by the client
        //    The OAuth server should respond with a redirect to the login page.
        let authorize_url = format!(
            "/oauth/authorize?response_type=code&redirect_uri={}&client_id={}&state={}",
            url(redirect_uri),
            url(client_id),
            url(state)
            );
        let response = http_client.get(authorize_url).dispatch();

        assert_eq!(response.status(), Status::SeeOther);
        let login_location = response.headers().get_one("Location").expect("Location header");
        assert!(login_location.starts_with("./login"));

        // 2. User requests the login page
        let login_url = format!("/oauth{}", &login_location[1..]);
        let mut response = http_client.get(login_url).dispatch();

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

        let mut response = http_client.post(login_url)
            .body(form_body)
            .header(ContentType::Form)
            .dispatch();

        assert_eq!(response.status(), Status::SeeOther);
        let grant_location = response.headers().get_one("Location").expect("Location header");
        assert!(grant_location.starts_with("./grant"));
        let session_cookie_str = response.headers().get_one("Set-Cookie").expect("Session cookie");
        let cookie_regex = Regex::new("^([^=]+)=([^;]+).*").unwrap();
        let (cookie_name, cookie_content) = cookie_regex.captures(&session_cookie_str)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .expect("session cookie");

        // 4. User requests grant page
        let grant_url = format!("/oauth{}", &grant_location[1..]);
        let cookie = Cookie::new(cookie_name, cookie_content);
        println!("Cookie sent: {:?}", cookie);
        let mut response = http_client.get(grant_url).cookie(cookie).dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::HTML));

        let state_regex = Regex::new("<input type=\"hidden\" name=\"state\" value=\"([^\"]+)\">").unwrap();
        let body = response.body_string().expect("response body");
        let form_state = state_regex.captures(&body).map(|c| c[1].to_string()).expect("hidden state field");

    }
}

