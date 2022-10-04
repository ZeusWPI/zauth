use crate::ephemeral::session::AdminSession;
use crate::models::user::User;
use crate::views::accepter::Accepter;

use rocket::form::Form;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::response::Responder;
use rocket::serde::json::Json;

#[get("/announcements/new")]
pub fn new_announcement<'r>(
	session: AdminSession,
) -> impl Responder<'r, 'static> {
	template! {"announcements/new.html"; current_user: User = session.admin }
}

#[derive(Debug, FromForm, Serialize, Deserialize)]
pub struct Announcement {
	subject: String,
	body:    String,
}

#[post("/announcements", data = "<announcement>")]
pub fn create_announcement<'r>(
	announcement: Form<Announcement>,
	session: AdminSession,
) -> impl Responder<'r, 'static> {
	Accepter {
		html: Custom(
			Status::Created,
			template!("announcements/created.html"; current_user: User = session.admin),
		),
		json: Custom(Status::Created, Json(())),
	}
}
