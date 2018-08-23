use rocket::Rocket;
use rocket::response::status::{BadRequest,NotFound};
use rocket::response::Redirect;
use rocket::request::Form;
use rocket::http::{Cookie, Cookies};
use rocket_contrib::Template;


#[derive(Debug, FromForm, Serialize)]
pub struct AuthorizationRequest {
    response_type: String,
    client_id: String,
    redirect_uri: Option<String>,
    scope: Option<String>,
    state: Option<String>
}

#[derive(Serialize)]
struct TemplateContext {
    auth_req: AuthorizationRequest
}

#[derive(Debug)]
struct Client {
    id : String,
    allowed_uris: Vec<String>
}

#[derive(Debug)]
struct User {
    username : String
}

impl Client {
    fn redirect_uri_acceptable(&self, redirect_uri : &Option<String>) -> bool {
        true
    }
}

pub fn mount(loc : &'static str, rocket : Rocket) -> Rocket {
    rocket.mount(loc, routes![authorize, authorize_parse_failed, login])
}

fn get_client_by_id(client_id : &String) -> Option<Client> {
    Some(Client {
        id: client_id.clone(),
        allowed_uris: Vec::new()
    })
}

fn get_user(username : &String, password : &String) -> Option<User> {
    Some(User{
        username : username.clone()
    })
}


#[get("/authorize?<req>")]
pub fn authorize(req : AuthorizationRequest, mut cookies : Cookies) -> Result<Template, NotFound<String>> {
    if let Some(client) = get_client_by_id(&req.client_id) {
        if client.redirect_uri_acceptable(&req.redirect_uri) {
            let context = TemplateContext {
                auth_req: req
            };
            Ok(Template::render("login", &context))
        } else {
            Err(NotFound(format!("Redirect uri '{:?}' is not allowed for client with id '{}'", req.redirect_uri, client.id)))
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
    client_id: String,
    scope: String,
    state: String,
    redirect_uri: String
}

#[post("/login", data="<form>")]
fn login(form : Form<LoginFormData>) -> String {
    let data = form.into_inner();
    if let Some(user) = get_user(&data.username, &data.password) {
        format!("User login correct: {:?}", user)
    } else {
        String::from("Nah")
    }
}
