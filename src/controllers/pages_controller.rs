use rocket::response::Responder;

use crate::ephemeral::session::UserSession;
use crate::models::client::Client;
use crate::models::user::User;
use crate::DbConn;

#[get("/")]
pub fn home_page(
	session: Option<UserSession>,
	conn: DbConn,
) -> impl Responder<'static>
{
	template! {
		"pages/home.html";
		current_user: Option<User> = session.map(|session| session.user),
		clients:      Vec<Client>  = Client::all(&conn),
		users:        Vec<User>    = User::all(&conn),
	}
}
