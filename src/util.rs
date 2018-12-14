use rocket::http::uri::Origin;
use rocket::response::Redirect;

use oauth::MountPoint;

pub fn redirect_to_relative(uri: Origin, mount_point: MountPoint) -> Redirect {
	// let mut uri_str = uri.to_string();
	// uri_str.remove(0);
	// println!("Redirect relative: {}", uri_str);
	Redirect::to(format!("{}{}", mount_point, uri))
}
