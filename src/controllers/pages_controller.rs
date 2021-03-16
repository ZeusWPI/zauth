use rocket::response::{Redirect, Responder};

use crate::controllers::users_controller::rocket_uri_macro_show_user;

use crate::ephemeral::session::UserSession;
use crate::errors::{Either, Result};
use crate::models::client::Client;
use crate::models::user::User;
use crate::DbConn;

#[get("/")]
pub fn home_page(
	session: Option<UserSession>,
	conn: DbConn,
) -> Result<Either<Redirect, impl Responder<'static>>> {
	match session {
		None => Ok(Either::Right(template! {
			"pages/home.html";
			clients:      Vec<Client>  = Client::all(&conn)?,
			users:        Vec<User>    = User::all(&conn)?,
		})),
		Some(session) => {
			Ok(Either::Left(Redirect::to(uri!(show_user: session.user.id))))
		},
	}
}
