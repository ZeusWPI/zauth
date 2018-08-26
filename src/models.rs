use rocket::http::{Cookie, Cookies};
use chrono::{DateTime, Local, Duration};

use oauth::*;

extern crate bincode;
extern crate serde_urlencoded;
extern crate base64;

#[derive(Serialize, Deserialize, Debug, FromForm)]
pub struct AuthState {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub client_state: Option<String>
}

impl AuthState {
    pub fn redirect_uri_with_state(&self) -> String {
        let state_param = self.client_state.as_ref().map_or(String::new(), |s| format!("state={}", s));
        format!("{}?{}", self.redirect_uri, state_param)
    }

    pub fn from_req(auth_req : AuthorizationRequest) -> AuthState {
        AuthState {
            client_id : auth_req.client_id,
            redirect_uri : auth_req.redirect_uri,
            scope : auth_req.scope,
            client_state : auth_req.state
        }
    }

    pub fn encode_url(&self) -> String {
        serde_urlencoded::to_string(self).unwrap()
    }

    pub fn decode_url(state_str : &str) -> Option<AuthState> {
        serde_urlencoded::from_str(state_str).ok()
    }

    pub fn encode_b64(&self) -> String {
        base64::encode(&bincode::serialize(self).unwrap())
    }

    pub fn decode_b64(state_str : &str) -> Option<AuthState> {
        bincode::deserialize(&base64::decode(state_str).ok().unwrap()).ok()
    }
}

#[derive(Serialize)]
pub struct TemplateContext {
    client_name : String,
    state : String,
}

impl TemplateContext {
    pub fn from_state(state : AuthState) -> TemplateContext {
        TemplateContext {
            client_name : state.client_id.clone(),
            state : state.encode_b64()
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

    pub fn redirect_uri_acceptable(&self, _redirect_uri : &str) -> bool {
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
    pub fn username(&self) -> &String {
        &self.username
    }

    pub fn find(username : &String) -> Option<User> {
        Some(User{
            username : username.clone()
        })
    }

    pub fn find_and_authenticate(username : &String, _password : &String) -> Option<User> {
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

    pub fn user(&self) -> User {
        User::find(&self.username)
            .expect("session for unexisting user")
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
