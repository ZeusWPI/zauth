extern crate serde_urlencoded;
extern crate chrono;
extern crate regex;
extern crate base64;

use rocket::Rocket;
use rocket::Outcome;
use rocket::http::Status;
use rocket::http::Cookies;
use rocket::response::status::{BadRequest,NotFound};
use rocket::response::Redirect;
use rocket::request::Form;
use rocket_contrib::Template;
use rocket::request::{self, Request, FromRequest};

use models::*;
use http_authentication::{BasicAuthentication};

pub const SESSION_VALIDITY_MINUTES : i64 = 60;

pub fn mount(loc : &'static str, rocket : Rocket) -> Rocket {
    rocket.mount(loc, routes![authorize, authorize_parse_failed, login_get, login_post, grant_get, grant_post, token])
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
            let state = State::from_req(req);
            Ok(Redirect::to(&format!("./login?{}", state.encode())))
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
fn login_get(state : State) -> Template {
    Template::render("login", TemplateContext::from_state(state))
}

#[post("/login", data="<form>")]
fn login_post(mut cookies : Cookies, form : Form<LoginFormData>) -> Result<Redirect, Template> {
    let data = form.into_inner();
    let state = State::decode(&data.state).unwrap();
    if let Some(user) = User::find_and_authenticate(&data.username, &data.password) {
        Session::add_to_cookies(&user, &mut cookies);
        Ok(Redirect::to(&format!("./grant?{}", data.state)))
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
fn grant_get(mut cookies : Cookies, state : State) -> Result<Template, String> {
    if let Some(_) = Session::from_cookies(&mut cookies) {
        Ok(Template::render("grant", TemplateContext::from_state(state)))
    } else {
        Err(String::from("No cookie :("))
    }
}

#[post("/grant", data="<form>")]
fn grant_post(mut cookies : Cookies, form : Form<GrantFormData>) -> Result<Redirect, String> {
    if let Some(session) = Session::from_cookies(&mut cookies) {
        let data = form.into_inner();
        let state = State::decode(&data.state).unwrap();
        if data.grant {
            Ok(authorization_granted(state, session.user()))
        } else {
            Ok(authorization_denied(state))
        }
    } else {
        Err(String::from("No cookie :("))
    }
}

fn authorization_granted(state : State, user : User) -> Redirect {
    // add code to state

    let authorization_code = "authorization_code";
    Redirect::to(&format!("{}&code={}", state.redirect_uri_with_state(), authorization_code))
}

fn authorization_denied(state : State) -> Redirect {
    Redirect::to(&format!("{}&error=access_denied", state.redirect_uri_with_state()))
}

#[derive(FromForm, Debug)]
struct TokenFormData {
    grant_type : String,
    code : String,
    redirect_uri : String,
    client_id : String
}


#[post("/token", data="<form>")]
fn token(auth : BasicAuthentication, form : Form<TokenFormData>) -> String {
    format!("{:?}", auth)
}



