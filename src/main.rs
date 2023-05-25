#[macro_use]
extern crate rocket;
extern crate zauth;

#[launch]
async fn zauth() -> _ {
	zauth::prepare()
}
