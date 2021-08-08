#[macro_use]
extern crate rocket;
extern crate zauth;

#[launch]
fn zauth() -> _ {
	zauth::prepare()
}
