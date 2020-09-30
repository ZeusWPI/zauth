use rocket::http::Cookies;
use rocket::request::Form;
use rocket::response::{Redirect, Responder};

use crate::controllers::oauth_controller::AuthState;
use crate::controllers::pages_controller::rocket_uri_macro_home_page;
use crate::ephemeral::session::{Session, UserSession};
use crate::errors::{Either, Result, ZauthError};
use crate::models::user::User;
use crate::DbConn;

#[get("/login?<state>")]
pub fn new_session(
	session: Option<UserSession>,
	state: Option<String>,
) -> Either<Redirect, impl Responder<'static>>
{
	match session {
		None => Either::Right(template! {
			"session/login.html";
			state: String = state.unwrap_or_default(),
			error: Option<String> = None
		}),
		Some(_user_session) => Either::Left(Redirect::to(uri!(home_page))),
	}
}

#[get("/logout")]
pub fn delete_session(session: UserSession) -> impl Responder<'static> {
	template! {
		"session/logout.html";
		current_user: User = session.user
	}
}

#[derive(FromForm, Debug)]
pub struct LoginFormData {
	username: String,
	password: String,
	state:    Option<String>,
}

#[post("/login", data = "<form>")]
pub fn create_session(
	form: Form<LoginFormData>,
	mut cookies: Cookies,
	conn: DbConn,
) -> Result<Either<Redirect, impl Responder<'static>>>
{
	let form = form.into_inner();
	match User::find_and_authenticate(&form.username, &form.password, &conn) {
		Err(ZauthError::LoginError(login_error)) => {
			Ok(Either::Right(template! {
				"session/login.html";
				state: String = form.state.unwrap(),
				error: Option<String> = Some(login_error.to_string()),
			}))
		},
		Ok(user) => {
			Session::add_to_cookies(user, &mut cookies);
			match AuthState::decode_b64(&form.state.unwrap_or_default()) {
				Ok(user_auth_state) => {
					Ok(Either::Left(Redirect::to(user_auth_state.redirect_uri)))
				},
				_ => Ok(Either::Left(Redirect::to("/"))),
			}
		},
		Err(err) => Err(err),
	}
}

#[post("/logout")]
pub fn destroy_session(mut cookies: Cookies) -> Redirect {
	Session::destroy(&mut cookies);
	Redirect::to("/")
}
