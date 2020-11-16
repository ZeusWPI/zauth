use rocket::http::Cookies;
use rocket::request::Form;
use rocket::response::{Redirect, Responder};

use crate::controllers::pages_controller::rocket_uri_macro_home_page;
use crate::ephemeral::session::{stored_redirect_or, Session, UserSession};
use crate::errors::{AuthenticationError, Either, Result, ZauthError};
use crate::models::user::User;
use crate::DbConn;

#[get("/login")]
pub fn new_session(
	session: Option<UserSession>,
	cookies: Cookies,
) -> Either<Redirect, impl Responder<'static>>
{
	match session {
		None => Either::Right(template! {
			"session/login.html";
			error: Option<String> = None
		}),
		_ => Either::Left(stored_redirect_or(cookies, uri!(home_page))),
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
				error: Option<String> = Some(login_error.to_string()),
			}))
		},
		Ok(user) => {
			Session::login(user, &mut cookies)?;
			Ok(Either::Left(stored_redirect_or(cookies, uri!(home_page))))
		},
		Err(err) => Err(err),
	}
}

#[post("/logout")]
pub fn destroy_session(mut cookies: Cookies) -> Redirect {
	Session::destroy(&mut cookies);
	Redirect::to("/")
}

#[get("/csrf_detected")]
pub fn csrf_detected() -> ZauthError {
	ZauthError::AuthError(AuthenticationError::DetectedCSRF)
}
