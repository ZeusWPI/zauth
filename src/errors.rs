error_chain! {
	foreign_links {
		SerdeUrlencode(serde_urlencoded::ser::Error);
	}
}
