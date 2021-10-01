use rocket::response::{Redirect, Responder};

use crate::controllers::users_controller::rocket_uri_macro_show_user;

use crate::ephemeral::session::UserSession;
use crate::errors::Either;

#[get("/")]
pub fn home_page<'r>(
	session: Option<UserSession>,
) -> Either<Redirect, impl Responder<'r, 'static>> {
	match session {
		None => Either::Right(template! {"pages/home.html"}),
		Some(session) => {
			Either::Left(Redirect::to(uri!(show_user(session.user.username))))
		},
	}
}
