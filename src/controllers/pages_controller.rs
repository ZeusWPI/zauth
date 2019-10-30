use rocket_contrib::templates::Template;

use ephemeral::session::UserSession;
use models::client::Client;
use models::user::User;
use DbConn;

#[derive(Serialize)]
pub struct HomeTemplate {
	current_user: Option<User>,
	clients:      Vec<Client>,
	users:        Vec<User>,
}

#[get("/")]
pub fn home_page(session: Option<UserSession>, conn: DbConn) -> Template {
	Template::render(
		"home",
		HomeTemplate {
			current_user: session.map(|session| session.user),
			clients:      Client::all(&conn),
			users:        User::all(&conn),
		},
	)
}
