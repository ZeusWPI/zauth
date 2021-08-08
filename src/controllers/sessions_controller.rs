use rocket::form::Form;
use rocket::response::{Redirect, Responder};

use crate::controllers::pages_controller::rocket_uri_macro_home_page;
use crate::ephemeral::session::{stored_redirect_or, Session, UserSession};
use crate::errors::{Either, Result, ZauthError};
use crate::models::user::User;
use crate::DbConn;
use rocket::http::CookieJar;

#[get("/login")]
pub fn new_session<'r>(
	session: Option<UserSession>,
	cookies: &CookieJar,
) -> Either<Redirect, impl Responder<'r, 'static>> {
	match session {
		None => Either::Right(template! {
			"session/login.html";
			error: Option<String> = None
		}),
		_ => Either::Left(stored_redirect_or(cookies, uri!(home_page))),
	}
}

#[get("/logout")]
pub fn delete_session<'r>(session: UserSession) -> impl Responder<'r, 'static> {
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
pub async fn create_session<'r>(
	form: Form<LoginFormData>,
	cookies: &'r CookieJar<'_>,
	db: DbConn,
) -> Result<Either<Redirect, impl Responder<'r, 'static>>> {
	let form = form.into_inner();
	match User::find_and_authenticate(form.username, form.password, &db).await {
		Err(ZauthError::LoginError(login_error)) => {
			Ok(Either::Right(template! {
				"session/login.html";
				error: Option<String> = Some(login_error.to_string()),
			}))
		},
		Ok(user) => {
			Session::login(user, cookies);
			Ok(Either::Left(stored_redirect_or(cookies, uri!(home_page))))
		},
		Err(err) => Err(err),
	}
}

#[post("/logout")]
pub fn destroy_session(cookies: &CookieJar) -> Redirect {
	Session::destroy(cookies);
	Redirect::to("/")
}
