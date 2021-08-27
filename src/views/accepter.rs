use rocket::http::{MediaType, QMediaType, Status};
use rocket::request::Request;
use rocket::response::status::Custom;
use rocket::response::{self, Responder};

pub struct Accepter<H, J> {
	pub html: H,
	pub json: J,
}

fn not_acceptable<'r>() -> impl Responder<'r, 'static> {
	Custom(Status::NotAcceptable, template!("errors/406.html"))
}

fn preferred_media<'r>(request: &'r Request<'_>) -> Vec<&'r MediaType> {
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

impl<'r, 'o: 'r, 'h: 'o, 'j: 'o, H, J> Responder<'r, 'o> for Accepter<H, J>
where
	H: Responder<'r, 'h>,
	J: Responder<'r, 'j>,
{
	fn respond_to(self, request: &'r Request<'_>) -> response::Result<'o> {
		let preferred = preferred_media(request);

		// No 'Accept' header given, return HTML by default
		if preferred.len() == 0 {
			return self.html.respond_to(request);
		}

		// Return first responder which maches
		for media in preferred {
			if media_types_match(media, &MediaType::HTML) {
				return self.html.respond_to(request);
			} else if media_types_match(media, &MediaType::JSON) {
				return self.json.respond_to(request);
			}
		}

		// No responder matched, return a 406.
		return not_acceptable().respond_to(request);
	}
}

#[cfg(test)]
#[allow(dead_code)]
mod test {

	use super::*;
	use rocket::http::{Accept, Header, Status};
	use rocket::local::blocking::Client;
	use rocket::response::content::Html;
	use rocket::response::Redirect;
	use rocket::serde::json::Json;

	#[get("/simple")]
	fn test_simple<'r>() -> impl Responder<'r, 'static> {
		Accepter {
			html: Html("<html><h1>Hello HTML"),
			json: Json(vec!["hello json"]),
		}
	}

	#[get("/redirect")]
	fn test_redirect() -> Accepter<Redirect, Redirect> {
		Accepter {
			html: Redirect::to(uri!(test_simple)),
			json: Redirect::to("/somewhere"),
		}
	}

	#[launch]
	fn rocket() -> _ {
		rocket::build().mount("/", routes![test_simple, test_redirect])
	}

	fn client() -> Client {
		Client::tracked(rocket()).expect("valid rocket")
	}

	#[test]
	fn accept_html() {
		let client = client();
		let response = client.get("/simple").header(Accept::HTML).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("text/html; charset=utf-8")
		);
		assert!(response
			.into_string()
			.expect("html body")
			.contains("Hello HTML"));
	}

	#[test]
	fn accept_json() {
		let client = client();
		let response = client.get("/simple").header(Accept::JSON).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("application/json")
		);
		assert_eq!(
			response.into_string().expect("json body"),
			"[\"hello json\"]"
		);
	}

	#[test]
	fn not_acceptable() {
		let client = client();
		let response = client.get("/simple").header(Accept::XML).dispatch();
		assert_eq!(response.status(), Status::NotAcceptable);
	}

	#[test]
	fn accept_anything() {
		let client = client();
		let response = client.get("/simple").header(Accept::Any).dispatch();
		assert_eq!(response.status(), Status::Ok);
	}

	#[test]
	fn prefers_html() {
		let client = client();
		let response = client
			.get("/simple")
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
			.into_string()
			.expect("html body")
			.contains("Hello HTML"));
	}

	#[test]
	fn prefers_json() {
		let client = client();
		let response = client
			.get("/simple")
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
			response.into_string().expect("json body"),
			"[\"hello json\"]"
		);
	}

	#[test]
	fn no_preference() {
		let client = client();

		let response = client.get("/simple").dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("text/html; charset=utf-8"),
			"should return HTML when no preference is given"
		);
	}

	#[test]
	fn route_redirect() {
		let client = client();
		let response = client.get("/redirect").dispatch();
		assert_eq!(response.status(), Status::SeeOther);

		let response = client.get("/redirect").header(Accept::HTML).dispatch();
		assert_eq!(response.status(), Status::SeeOther);

		let response = client.get("/redirect").header(Accept::JSON).dispatch();
		assert_eq!(response.status(), Status::SeeOther);

		let response = client.get("/redirect").header(Accept::XML).dispatch();
		assert_eq!(response.status(), Status::NotAcceptable);
	}
}
