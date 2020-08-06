use askama::Template;
use rocket::http::Cookies;
use rocket::http::Status;
use rocket::request::Form;
use rocket::response::{status, Redirect, Responder};

use crate::ephemeral::session::Session;
use crate::models::user::User;
use crate::DbConn;

#[get("/login?<state>")]
pub fn new_session(state: Option<String>) -> impl Responder<'static> {
	template! {
		"session/login.html";
		state: String = state.unwrap_or_default(),
		error: Option<String> = None
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
	state: Option<String>,
}

#[post("/login", data = "<form>")]
pub fn create_session(
	form: Form<LoginFormData>,
	mut cookies: Cookies,
	conn: DbConn,
) -> impl Responder<'static> {
	let form = form.into_inner();
	let user =
		User::find_and_authenticate(&form.username, &form.password, &conn);

	// TODO: handle error value better
	if let Ok(user) = user {
		Session::add_to_cookies(user, &mut cookies);
		Ok(Redirect::to("/"))
	} else {
		Err(status::Custom(
			Status::Unauthorized,
			template! {
			"session/login.html";
			state: String = form.state.unwrap_or_default(),
			error: Option<String> =
				Some(String::from("Incorrect username or password")),
			},
		))
	}
}

#[post("/logout")]
pub fn destroy_session(mut cookies: Cookies) -> Redirect {
	Session::destroy(&mut cookies);
	Redirect::to("/")
}
