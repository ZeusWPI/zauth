use rocket::http::Cookies;
use rocket::http::Status;
use rocket::request::Form;
use rocket::response::status;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;
use std::collections::HashMap;

use ephemeral::session::Session;
use models::user::User;
use DbConn;

#[derive(Serialize)]
pub struct LoginTemplate {
	state: Option<String>,
	error: Option<String>,
}

#[get("/login?<state>")]
pub fn new_session(state: Option<String>) -> Template {
	Template::render("session/login", LoginTemplate { state, error: None })
}

#[get("/logout")]
pub fn delete_session() -> Template {
	Template::render("session/logout", HashMap::<String, String>::new())
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
			Template::render(
				"session/login",
				LoginTemplate {
					state: form.state,
					error: Some(String::from("Incorrect username or password")),
				},
			),
		))
	}
}

#[post("/logout")]
pub fn destroy_session(mut cookies: Cookies) -> Redirect {
	Session::destroy(&mut cookies);
	Redirect::to("/")
}
