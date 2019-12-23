use rocket::http::{QMediaType, Status};
use rocket::request::Request;
use rocket::response::{self, Responder, Response};

#[macro_export]
macro_rules! template {
	($template_name:literal) => {
		{
			use rocket_contrib::templates::Template;
			#[derive(Serialize)]
			struct TemplateStruct {};
			Template::render($template_name, TemplateStruct{})
		}
	};
	($template_name:literal; $($name:ident: $type:ty = $value:expr),+$(,)?) => {
		{
			use rocket_contrib::templates::Template;
			#[derive(Serialize)]
			struct TemplateStruct {
				$(
					$name: $type,
				)+
			}
			Template::render(
				$template_name,
				TemplateStruct {
					$(
						$name: $value,
					)+
				}
			)
		}
	}
}

type FnRespond<'r> = Box<dyn FnOnce(&Request) -> response::Result<'r>>;

pub struct View<'r> {
	json: Option<FnRespond<'r>>,
	html: Option<FnRespond<'r>>,
}

impl<'r> View<'r> {
	pub fn new() -> Self {
		View {
			json: None,
			html: None,
		}
	}

	pub fn json(mut self, respond: FnRespond<'r>) -> Self {
		self.json = Some(respond);
		self
	}

	pub fn html(mut self, respond: FnRespond<'r>) -> Self {
		self.html = Some(respond);
		self
	}
}

impl<'r> Responder<'r> for View<'r> {
	fn respond_to(self, request: &Request) -> response::Result<'r> {
		request
			.accept()
			.and_then(|accept| {
				let mut accepts = accept.iter().collect::<Vec<&QMediaType>>();
				accepts.sort_by(|p, q| {
					let pw = p.weight_or(1.0);
					let qw = q.weight_or(1.0);
					pw.partial_cmp(&qw).unwrap_or(std::cmp::Ordering::Less)
				});
				for qmedia in accepts {
					if qmedia.is_json() {
						return self.json;
					} else if qmedia.is_html() {
						return self.html;
					}
				}
				None
			})
			.map(|respond| respond(request))
			.unwrap_or_else(|| {
				let template = template!("errors/406").respond_to(request)?;
				Ok(Response::build_from(template)
					.status(Status::NotAcceptable)
					.finalize())
			})
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use rocket::http::Status;
	use rocket::local::Client;
	use rocket_contrib::templates::Template;

	#[get("/nothing")]
	fn view_nothing<'r>() -> View<'r> {
		View::new()
	}

	fn client() -> Client {
		Client::new(
			rocket::ignite()
				.attach(Template::fairing())
				.mount("/", routes![view_nothing]),
		)
		.unwrap()
	}

	#[test]
	fn test_nothing() {
		let client = client();
		let mut response = client.get("/nothing").dispatch();
		assert_eq!(response.status(), Status::NotAcceptable);
		assert!(response
			.body_string()
			.expect("html body")
			.contains("<h1>Not acceptable</h1>"));
	}
}
