use rocket::Rocket;
use rocket::http::Cookies;
use rocket::response::status::{BadRequest,NotFound};
use rocket::response::Redirect;
use rocket::request::Form;
use rocket_contrib::Template;

use models::*;

extern crate serde_urlencoded;
extern crate chrono;

pub const SESSION_VALIDITY_MINUTES : i64 = 60;

pub fn mount(loc : &'static str, rocket : Rocket) -> Rocket {
    rocket.mount(loc, routes![authorize, authorize_parse_failed, login_get, login_post, grant_get, grant_post])
}

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: Option<String>,
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
    if let Some(user) = User::find(&data.username, &data.password) {
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

#[post("/grant", data="<data>")]
fn grant_post(mut cookies : Cookies, data : Form<GrantFormData>) -> String {
    if let Some(_) = Session::from_cookies(&mut cookies) {
        String::from(format!("{:?}", data.into_inner()))
    } else {
        String::from("No cookie :(")
    }
}
