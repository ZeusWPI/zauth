use rocket::http::{Cookie, CookieJar, Status};
use rocket::outcome::try_outcome;
use rocket::request::{FromRequest, Outcome, Request};
use std::str::FromStr;

use crate::DbConn;
use crate::controllers::sessions_controller::rocket_uri_macro_new_session;
use crate::errors::Result;
use crate::models::client::Client;
use crate::models::session::Session;
use crate::models::user::User;
use rocket::http::uri::Origin;
use rocket::response::Redirect;

const REDIRECT_COOKIE: &str = "ZAUTH_REDIRECT";
const SESSION_COOKIE: &str = "ZAUTH_SESSION";

pub fn ensure_logged_in_and_redirect(
	cookies: &CookieJar,
	uri: Origin,
) -> Redirect {
	cookies.add_private(Cookie::new(REDIRECT_COOKIE, uri.to_string()));
	Redirect::to(uri!(new_session))
}

pub fn stored_redirect_or(cookies: &CookieJar, fallback: Origin) -> Redirect {
	let location: Origin =
		if let Some(cookie) = cookies.get_private(REDIRECT_COOKIE) {
			let stored = Origin::parse_owned(String::from(cookie.value())).ok();
			cookies.remove_private(cookie);
			stored.unwrap_or(fallback)
		} else {
			fallback
		};
	Redirect::to(location.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionCookie {
	session_id: i32,
}

impl SessionCookie {
	pub fn new(session: Session) -> SessionCookie {
		SessionCookie {
			session_id: session.id,
		}
	}

	pub fn login(self, cookies: &CookieJar) {
		let session_str = serde_urlencoded::to_string(self).unwrap();
		let session_cookie = Cookie::new(SESSION_COOKIE, session_str);
		cookies.add_private(session_cookie);
	}

	pub async fn session(&self, db: &DbConn) -> Result<Session> {
		Session::find_by_id(self.session_id, db).await
	}
}

impl FromStr for SessionCookie {
	type Err = serde_urlencoded::de::Error;

	fn from_str(cookie: &str) -> std::result::Result<SessionCookie, Self::Err> {
		serde_urlencoded::from_str(cookie)
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SessionCookie {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
		let session = request
			.cookies()
			.get_private(SESSION_COOKIE)
			.map(|cookie| SessionCookie::from_str(cookie.value()));
		match session {
			Some(Ok(session)) => Outcome::Success(session),
			_ => Outcome::Error((Status::Unauthorized, "invalid session")),
		}
	}
}

#[derive(Debug)]
pub struct UserSession {
	pub user: User,
	session: Session,
}

impl UserSession {
	pub async fn destroy(
		self,
		cookies: &CookieJar<'_>,
		db: &DbConn,
	) -> Result<()> {
		cookies.remove_private(Cookie::from(SESSION_COOKIE));
		self.session.invalidate(db).await?;
		Ok(())
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserSession {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
		let cookie = try_outcome!(request.guard::<SessionCookie>().await);
		let db =
			try_outcome!(request.guard::<DbConn>().await.map_error(|_| {
				(Status::InternalServerError, "could not connect to database")
			}));

		match Session::find_by_id(cookie.session_id, &db).await {
			Ok(session) => match session.user(&db).await {
				Ok(user) => Outcome::Success(UserSession { user, session }),
				_ => Outcome::Error((
					Status::Unauthorized,
					"user not found for database session",
				)),
			},
			_ => Outcome::Error((
				Status::Unauthorized,
				"session not found for valid cookie",
			)),
		}
	}
}

#[derive(Debug)]
pub struct AdminSession {
	pub admin: User,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminSession {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
		let session = try_outcome!(request.guard::<UserSession>().await);
		let user: User = session.user;
		if user.admin {
			Outcome::Success(AdminSession { admin: user })
		} else {
			Outcome::Error((Status::Forbidden, "user is not an admin"))
		}
	}
}

#[derive(Debug)]
pub struct ClientSession {
	pub user: User,
	pub client: Client,
	pub scope: Option<String>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientSession {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
		let headers: Vec<_> = request.headers().get("Authorization").collect();
		if headers.is_empty() {
			return Outcome::Error((
				Status::BadRequest,
				"no authorization header found",
			));
		} else if headers.len() > 1 {
			return Outcome::Error((
				Status::BadRequest,
				"found more than one authorization header",
			));
		}

		let auth_header = headers[0];
		let prefix = "Bearer ";
		if !auth_header.starts_with(prefix) {
			return Outcome::Error((
				Status::BadRequest,
				"only support Bearer tokens are supported",
			));
		}
		let key = &auth_header[prefix.len()..];

		let db =
			try_outcome!(request.guard::<DbConn>().await.map_error(|_| {
				(Status::InternalServerError, "could not connect to database")
			}));

		match Session::find_by_key(key.to_string(), &db).await {
			Ok(session) => match session.user(&db).await {
				Ok(user) => match session.client(&db).await {
					Ok(Some(client)) => Outcome::Success(ClientSession {
						user,
						client,
						scope: session.scope,
					}),
					_ => Outcome::Error((
						Status::Unauthorized,
						"there is no client associated to this client session",
					)),
				},
				_ => Outcome::Error((
					Status::Unauthorized,
					"user not found for database session",
				)),
			},
			_ => Outcome::Error((
				Status::Unauthorized,
				"session not found for valid cookie",
			)),
		}
	}
}

#[derive(Debug)]
pub struct ClientOrUserSession {
	pub user: User,
	pub client: Option<Client>,
	pub scope: Option<String>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientOrUserSession {
	type Error = &'static str;

	async fn from_request(
		request: &'r Request<'_>,
	) -> Outcome<Self, Self::Error> {
		match request.guard::<UserSession>().await {
			Outcome::Success(session) => {
				Outcome::Success(ClientOrUserSession {
					user: session.user,
					client: None,
					scope: None,
				})
			},
			_ => match request.guard::<ClientSession>().await {
				Outcome::Success(session) => {
					Outcome::Success(ClientOrUserSession {
						user: session.user,
						client: Some(session.client),
						scope: session.scope,
					})
				},
				_ => Outcome::Error((
					Status::Unauthorized,
					"found neither a user session or client session",
				)),
			},
		}
	}
}
