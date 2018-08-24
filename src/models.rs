use rocket::http::{Cookie, Cookies};
use chrono::{DateTime, Local, Duration};

use oauth::*;

extern crate serde_urlencoded;

#[derive(Serialize, Deserialize, Debug, FromForm)]
pub struct State {
    client_id: String,
    redirect_uri: Option<String>,
    scope: Option<String>,
    state: Option<String>
}

impl State {
    pub fn from_req(auth_req : AuthorizationRequest) -> State {
        State {
            client_id : auth_req.client_id,
            redirect_uri : auth_req.redirect_uri,
            scope : auth_req.scope,
            state : auth_req.state
        }
    }

    pub fn encode(&self) -> String {
        serde_urlencoded::to_string(self).unwrap()
    }

    pub fn decode(state_str : &str) -> Option<State> {
        serde_urlencoded::from_str(state_str).ok()
    }
}

#[derive(Serialize)]
pub struct TemplateContext {
    client_name : String,
    state : String,
}

impl TemplateContext {
    pub fn from_state(state : State) -> TemplateContext {
        TemplateContext {
            client_name : state.client_id.clone(),
            state : state.encode()
        }
    }
}

#[derive(Debug)]
pub struct Client {
    id : String,
    allowed_uris: Vec<String>
}


impl Client {
    pub fn id(&self) -> &String {
        &self.id
    }

    pub fn redirect_uri_acceptable(&self, _redirect_uri : &Option<String>) -> bool {
        true
    }

    pub fn find(client_id : &String) -> Option<Client> {
        Some(Client {
            id: client_id.clone(),
            allowed_uris: Vec::new()
        })
    }
}

#[derive(Debug)]
pub struct User {
    username : String
}

impl User {
    pub fn find(username : &String, _password : &String) -> Option<User> {
        Some(User{
            username : username.clone()
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    username : String,
    expiry : DateTime<Local>
}

impl Session {
    pub fn new(username : String) -> Session {
        let expiry = Local::now() + Duration::minutes(SESSION_VALIDITY_MINUTES);
        Session {
            username,
            expiry
        }
    }

    pub fn add_to_cookies(user : &User, cookies : &mut Cookies) {
        let session = Session::new(user.username.clone());
        let session_str = serde_urlencoded::to_string(session).unwrap();
        let session_cookie = Cookie::new("session", session_str);
        cookies.add_private(session_cookie);
    }

    pub fn from_cookies(cookies : &mut Cookies) -> Option<Session> {
        cookies.get_private("session")
               .map_or(None, |cookie| serde_urlencoded::from_str(cookie.value()).ok())
    }
}
