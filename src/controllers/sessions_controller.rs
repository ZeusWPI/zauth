use rocket::http::Cookies;
use rocket::http::Status;
use rocket::request::Form;
use rocket::response::status;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;

use crate::ephemeral::session::Session;
use crate::models::user::User;
use crate::DbConn;

#[get("/login?<state>")]
pub fn new_session(state: Option<String>) -> Template {
	template! {
		"session/login";
		state: Option<String> = state,
		error: Option<String> = None
	}
}

#[get("/logout")]
pub fn delete_session() -> Template {
	template! {"session/logout"}
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
) -> Result<Redirect, status::Custom<Template>>
{
	let form = form.into_inner();
	let user =
		User::find_and_authenticate(&form.username, &form.password, &conn);
	if let Some(user) = user {
		Session::add_to_cookies(user, &mut cookies);
		Ok(Redirect::to("/"))
	} else {
		Err(status::Custom(
			Status::Unauthorized,
			template! {
			"session/login";
			state: Option<String> = form.state,
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
