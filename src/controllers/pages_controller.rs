use rocket::response::{Redirect, Responder};

use crate::controllers::users_controller::rocket_uri_macro_show_user;

use crate::ephemeral::session::UserSession;
use crate::errors::{Either, Result};
use crate::models::client::Client;
use crate::models::user::User;
use crate::DbConn;

#[get("/")]
pub async fn home_page<'r>(
	session: Option<UserSession>,
	db: DbConn,
) -> Result<Either<Redirect, impl Responder<'r, 'static>>> {
	match session {
		None => Ok(Either::Right(template! {
			"pages/home.html";
			clients:      Vec<Client>  = Client::all(&db).await?,
			users:        Vec<User>    = User::all(&db).await?,
		})),
		Some(session) => {
			Ok(Either::Left(Redirect::to(uri!(show_user(session.user.id)))))
		},
	}
}
