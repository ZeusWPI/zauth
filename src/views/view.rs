use rocket::http::{MediaType, QMediaType};
use rocket::request::Request;
use rocket::response::{self, Responder};

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

pub struct Accepter<H, J, D> {
	html:    H,
	json:    J,
	default: D,
}

fn preferred_media<'r>(request: &'r Request) -> Vec<&'r MediaType> {
	request
		.accept()
		.map(|accept| {
			let mut accepts = accept.iter().collect::<Vec<&QMediaType>>();
			accepts.sort_by(|p, q| {
				let pw = p.weight_or(1.0);
				let qw = q.weight_or(1.0);
				qw.partial_cmp(&pw).unwrap_or(std::cmp::Ordering::Less)
			});
			accepts
				.iter()
				.map(|qmedia| qmedia.media_type())
				.collect::<Vec<&'r MediaType>>()
		})
		.unwrap_or_else(Vec::new)
}

fn media_types_match(first: &MediaType, other: &MediaType) -> bool {
	let collide = |a, b| a == "*" || b == "*" || a == b;
	collide(first.top(), other.top()) && collide(first.sub(), other.sub())
}

impl<'r, H: Responder<'r>, J: Responder<'r>, D: Responder<'r>> Responder<'r>
	for Accepter<H, J, D>
{
	fn respond_to(self, request: &Request) -> response::Result<'r> {
		for media in preferred_media(request) {
			if media_types_match(media, &MediaType::HTML) {
				return self.html.respond_to(request);
			} else if media_types_match(media, &MediaType::JSON) {
				return self.json.respond_to(request);
			}
		}
		return self.default.respond_to(request);
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use rocket::http::{Accept, Header, MediaType, Status};
	use rocket::local::Client;
	use rocket::response::content::Html;
	use rocket::response::status::Custom;
	use rocket_contrib::json::Json;
	use rocket_contrib::templates::Template;

	#[get("/test")]
	fn test_view() -> Accepter<
		Html<&'static str>,
		Json<Vec<&'static str>>,
		Custom<&'static str>,
	> {
		Accepter {
			json:    Json(vec!["hello json"]),
			html:    Html("<html><h1>Hello HTML"),
			default: Custom(Status::NotAcceptable, "not acceptable"),
		}
	}

	fn client() -> Client {
		Client::new(
			rocket::ignite()
				.attach(Template::fairing())
				.mount("/", routes![test_view]),
		)
		.unwrap()
	}

	#[test]
	fn accept_html() {
		let client = client();
		let mut response = client.get("/test").header(Accept::HTML).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("text/html; charset=utf-8")
		);
		assert!(response
			.body_string()
			.expect("html body")
			.contains("Hello HTML"));
	}

	#[test]
	fn accept_json() {
		let client = client();
		let mut response = client.get("/test").header(Accept::JSON).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("application/json")
		);
		assert_eq!(
			response.body_string().expect("json body"),
			"[\"hello json\"]"
		);
	}

	#[test]
	fn not_acceptable() {
		let client = client();
		let response = client.get("/test").header(Accept::XML).dispatch();
		assert_eq!(response.status(), Status::NotAcceptable);
	}

	#[test]
	fn accept_anything() {
		let client = client();
		let response = client.get("/test").header(Accept::Any).dispatch();
		assert_eq!(response.status(), Status::Ok);
	}

	#[test]
	fn preffers_html() {
		let client = client();
		let mut response = client
			.get("/test")
			.header(Header::new(
				"Accept",
				"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;\
				 q=0.8",
			))
			.dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("text/html; charset=utf-8")
		);
		assert!(response
			.body_string()
			.expect("html body")
			.contains("Hello HTML"));
	}

	#[test]
	fn preffers_json() {
		let client = client();
		let mut response = client
			.get("/test")
			.header(Header::new(
				"Accept",
				"bloep/bliep;q=0.9,application/json;q=0.8,*/*;q=0.8",
			))
			.dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("application/json")
		);
		assert_eq!(
			response.body_string().expect("json body"),
			"[\"hello json\"]"
		);
	}
}
