use rocket::http::Cookies;
use rocket::request::Form;
use rocket::response::{Redirect, Responder};

use crate::controllers::pages_controller::{rocket_uri_macro_home_page};
use crate::ephemeral::session::{Session, UserSession};
use crate::errors::{Either, Result};
use crate::models::user::User;
use crate::DbConn;

#[get("/login?<state>")]
pub fn new_session(
	session: Option<UserSession>,
	state: Option<String>,
) -> Either<Redirect, impl Responder<'static>>
{
	match session.map(|session| session.user) {
		None => {
			Either::Right(template! {
				"session/login.html";
				state: String = state.unwrap_or_default(),
				error: Option<String> = None
			})
		},
		Some(userSession) => Either::Left(Redirect::to(uri!(home_page))),
	}
}

#[get("/logout")]
pub fn delete_session() -> impl Responder<'static> {
	template! {"session/logout.html"}
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
	let user =
		User::find_and_authenticate(&form.username, &form.password, &conn)?;

	if let Some(user) = user {
		Session::add_to_cookies(user, &mut cookies);
		Ok(Either::Left(Redirect::to("/")))
	} else {
		Ok(Either::Right(template! {
			"session/login.html";
			state: String = form.state.unwrap(),
			error: Option<String> = Some(String::from("Username or password incorrect")),
		}))
	}
}

#[post("/logout")]
pub fn destroy_session(mut cookies: Cookies) -> Redirect {
	Session::destroy(&mut cookies);
	Redirect::to("/")
}
