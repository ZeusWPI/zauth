use rocket_contrib::templates::Template;

use crate::ephemeral::session::UserSession;
use crate::models::client::Client;
use crate::models::user::User;
use crate::DbConn;

#[derive(Serialize)]
pub struct HomeTemplate {
	current_user: Option<User>,
	clients:      Vec<Client>,
	users:        Vec<User>,
}

#[get("/")]
pub fn home_page(session: Option<UserSession>, conn: DbConn) -> Template {
	Template::render(
		"pages/home",
		HomeTemplate {
			current_user: session.map(|session| session.user),
			clients:      Client::all(&conn),
			users:        User::all(&conn),
		},
	)
}
