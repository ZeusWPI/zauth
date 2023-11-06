use std::fs::File;
use std::io::Write;
use std::path::Path;

use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::pkey::PKey;

fn main() {
	let path = Path::new("keys/jwt_key.pem");
	if !path.exists() {
		let group = EcGroup::from_curve_name(Nid::SECP384R1).unwrap();
		let pkey = PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap();
		let mut f = File::create(path).unwrap();
		let pem = pkey.private_key_to_pem_pkcs8().unwrap();
		f.write_all(&pem).unwrap();
	}
}
