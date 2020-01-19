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
	};
}

#[macro_export]
macro_rules! view {
	(html: $html:expr, json: $json:expr$(,)?) => {{
		use rocket::http::Status;
		use rocket::response::status::Custom;
		let not_acceptable =
			Custom(Status::NotAcceptable, template!("errors/406"));
		view!(html: $html, json: $json, default: not_acceptable)
		}};

	(html: $html:expr, json: $json:expr, default: $default:expr$(,)?) => {{
		use crate::views::accepter::Accepter;
		Accepter {
			html:    $html,
			json:    $json,
			default: $default,
			}
		}};
}

#[cfg(test)]
mod test {
	use super::*;
	use rocket::http::{Accept, Header, MediaType, Status};
	use rocket::local::Client;
	use rocket::response::content::Html;
	use rocket::response::status::Custom;
	use rocket::response::{Redirect, Responder};
	use rocket_contrib::json::Json;
	use rocket_contrib::templates::Template;

	#[get("/simple")]
	fn test_simple<'r>() -> impl Responder<'static> {
		view!(
			html: Html("<html><h1>Hello HTML"),
			json: Json(vec!["hello json"]),
		)
	}

	#[get("/default")]
	fn test_default() -> impl Responder<'static> {
		view!(
			html: Html("<html><h1>Hello HTML"),
			json: Json(vec!["hello json"]),
			default: String::from("default"),
		)
	}

	#[get("/redirect")]
	fn test_redirect<'r>() -> impl Responder<'static> {
		view!(
			html: Redirect::to(uri!(test_default)),
			json: Redirect::to("/somewhere"),
		)
	}

	fn client() -> Client {
		Client::new(
			rocket::ignite()
				.attach(Template::fairing())
				.mount("/", routes![test_simple, test_default, test_redirect]),
		)
		.unwrap()
	}

	#[test]
	fn route_simple() {
		let client = client();
		let response = client.get("/simple").header(Accept::HTML).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("text/html; charset=utf-8")
		);

		let response = client.get("/simple").header(Accept::JSON).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("application/json")
		);

		let response = client.get("/simple").header(Accept::XML).dispatch();
		assert_eq!(response.status(), Status::NotAcceptable);
	}

	#[test]
	fn route_with_default() {
		let client = client();
		let response = client.get("/default").header(Accept::HTML).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("text/html; charset=utf-8")
		);

		let response = client.get("/default").header(Accept::JSON).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("application/json")
		);

		let response = client.get("/default").header(Accept::XML).dispatch();
		assert_eq!(response.status(), Status::Ok);
		assert_eq!(
			response.headers().get("content-type").next(),
			Some("text/plain; charset=utf-8")
		);
	}

	#[test]
	fn route_redirect() {
		let client = client();
		let response = client.get("/redirect").header(Accept::HTML).dispatch();
		assert_eq!(response.status(), Status::SeeOther);

		let response = client.get("/redirect").header(Accept::JSON).dispatch();
		assert_eq!(response.status(), Status::SeeOther);

		let response = client.get("/redirect").header(Accept::XML).dispatch();
		assert_eq!(response.status(), Status::NotAcceptable);
	}
}
