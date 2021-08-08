use rocket::data::{Data, FromData, Outcome};
use rocket::form::Form;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::Request;

#[derive(Debug)]
pub struct Api<T> {
	inner: T,
}

impl<T: std::fmt::Debug> Api<T> {
	pub fn into_inner(self) -> T {
		self.inner
	}
}

#[derive(Debug)]
pub enum ApiError<'r, T>
where
	Form<T>: FromData<'r>,
	Json<T>: FromData<'r>,
	T: std::fmt::Debug,
{
	FormError(<Form<T> as FromData<'r>>::Error),
	JsonError(<Json<T> as FromData<'r>>::Error),
	WasNeither,
}

#[rocket::async_trait]
impl<'r, T: 'r> FromData<'r> for Api<T>
where
	Form<T>: FromData<'r>,
	Json<T>: FromData<'r>,
	T: std::fmt::Debug,
{
	type Error = ApiError<'r, T>;

	async fn from_data(
		request: &'r Request<'_>,
		data: Data<'r>,
	) -> Outcome<'r, Self> {
		if request.content_type() == Some(&ContentType::Form) {
			Form::from_data(request, data)
				.await
				.map(|v| Api {
					inner: v.into_inner(),
				})
				.map_failure(|(s, e)| (s, ApiError::FormError(e)))
		} else if request.content_type() == Some(&ContentType::JSON) {
			Json::from_data(request, data)
				.await
				.map(|v| Api {
					inner: v.into_inner(),
				})
				.map_failure(|(s, e)| (s, ApiError::JsonError(e)))
		} else {
			Outcome::Failure((Status::NotAcceptable, ApiError::WasNeither))
		}
	}
}
