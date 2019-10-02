#[derive(Debug)]
pub struct User {
	username: String,
}

impl User {
	pub fn username(&self) -> &String {
		&self.username
	}

	pub fn find(username: &String) -> Option<User> {
		Some(User {
			username: username.clone(),
		})
	}

	pub fn find_and_authenticate(
		username: &String,
		_password: &String,
	) -> Option<User>
	{
		Some(User {
			username: username.clone(),
		})
	}
}
