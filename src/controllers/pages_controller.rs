use rocket_contrib::templates::Template;

use crate::ephemeral::session::UserSession;
use crate::models::client::Client;
use crate::models::user::User;
use crate::DbConn;

#[get("/")]
pub fn home_page(session: Option<UserSession>, conn: DbConn) -> Template {
	template! {
		"pages/home";
		current_user: Option<User> = session.map(|session| session.user),
		clients:      Vec<Client>  = Client::all(&conn),
		users:        Vec<User>    = User::all(&conn),
	}
}
