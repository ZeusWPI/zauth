#[derive(Debug)]
pub struct Client {
	pub id:           String,
	pub allowed_uris: Vec<String>,
}

impl Client {
	pub fn id(&self) -> &String {
		&self.id
	}

	pub fn needs_grant(&self) -> bool {
		true
	}

	pub fn redirect_uri_acceptable(&self, _redirect_uri: &str) -> bool {
		true
	}

	pub fn find(client_id: &String) -> Option<Client> {
		Some(Client {
			id:           client_id.clone(),
			allowed_uris: Vec::new(),
		})
	}

	pub fn find_and_authenticate(
		client_id: &String,
		_secret: &str,
	) -> Option<Client>
	{
		Some(Client {
			id:           client_id.clone(),
			allowed_uris: Vec::new(),
		})
	}
}
