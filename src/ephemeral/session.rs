use chrono::{DateTime, Duration, Local};
use rocket::http::{Cookie, Cookies};

use models::user::User;
use DbConn;

pub const SESSION_VALIDITY_MINUTES: i64 = 60;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
	user_id: i32,
	expiry:  DateTime<Local>,
}

impl Session {
	pub fn new(user: User) -> Session {
		Session {
			user_id: user.id,
			expiry:  Local::now() + Duration::minutes(SESSION_VALIDITY_MINUTES),
		}
	}

	pub fn add_to_cookies(user: User, cookies: &mut Cookies) {
		let session = Session::new(user);
		let session_str = serde_urlencoded::to_string(session).unwrap();
		let session_cookie = Cookie::new("session", session_str);
		cookies.add_private(session_cookie);
	}

	pub fn user_from_cookies(
		cookies: &mut Cookies,
		conn: &DbConn,
	) -> Option<User>
	{
		cookies
			.get_private("session")
			.and_then(|cookie| serde_urlencoded::from_str(cookie.value()).ok())
			.and_then(|session: Self| session.user(conn))
	}

	fn user(&self, conn: &DbConn) -> Option<User> {
		if Local::now() > self.expiry {
			None
		} else {
			User::find(*&self.user_id, conn)
		}
	}
}
