use rocket::data::{FromData, Outcome, Transform, Transformed};
use rocket::http::{ContentType, Status};
use rocket::request::Form;
use rocket::{Data, Request};
use rocket_contrib::json::Json;

#[derive(Debug)]
pub struct Api<T> {
	inner: T,
}

impl<T> Api<T> {
	pub fn into_inner(self) -> T {
		self.inner
	}
}

pub enum ApiError<'a, T>
where
	Form<T>: FromData<'a>,
	Json<T>: FromData<'a>,
{
	FormError(<Form<T> as FromData<'a>>::Error),
	JsonError(<Json<T> as FromData<'a>>::Error),
	WasNeither,
}

impl<'a, T: 'a> FromData<'a> for Api<T>
where
	Form<T>: FromData<'a, Owned = String, Borrowed = str>,
	Json<T>: FromData<'a, Owned = String, Borrowed = str>,
{
	type Borrowed = str;
	type Error = ApiError<'a, T>;
	type Owned = String;

	fn transform(
		request: &Request,
		data: Data,
	) -> Transform<Outcome<Self::Owned, Self::Error>>
	{
		if request.content_type() == Some(&ContentType::Form) {
			match Form::transform(request, data) {
				Transform::Borrowed(o) => Transform::Borrowed(
					o.map_failure(|(s, e)| (s, ApiError::FormError(e))),
				),
				Transform::Owned(o) => Transform::Owned(
					o.map_failure(|(s, e)| (s, ApiError::FormError(e))),
				),
			}
		} else if request.content_type() == Some(&ContentType::JSON) {
			match Json::transform(request, data) {
				Transform::Borrowed(o) => Transform::Borrowed(
					o.map_failure(|(s, e)| (s, ApiError::JsonError(e))),
				),
				Transform::Owned(o) => Transform::Owned(
					o.map_failure(|(s, e)| (s, ApiError::JsonError(e))),
				),
			}
		} else {
			Transform::Owned(Outcome::Failure((
				Status::UnprocessableEntity,
				ApiError::WasNeither,
			)))
		}
	}

	fn from_data(
		request: &Request,
		outcome: Transformed<'a, Self>,
	) -> Outcome<Self, Self::Error>
	{
		if request.content_type() == Some(&ContentType::Form) {
			let outcome = match outcome {
				Transform::Borrowed(o) => {
					Transform::Borrowed(o.map_failure(|(s, e)| match e {
						ApiError::FormError(e) => (s, e),
						_ => unreachable!(),
					}))
				},
				Transform::Owned(o) => {
					Transform::Owned(o.map_failure(|(s, e)| match e {
						ApiError::FormError(e) => (s, e),
						_ => unreachable!(),
					}))
				},
			};
			Form::from_data(request, outcome)
				.map(|v| Api {
					inner: v.into_inner(),
				})
				.map_failure(|(s, e)| (s, ApiError::FormError(e)))
		} else if request.content_type() == Some(&ContentType::JSON) {
			let outcome = match outcome {
				Transform::Borrowed(o) => {
					Transform::Borrowed(o.map_failure(|(s, e)| match e {
						ApiError::JsonError(e) => (s, e),
						_ => unreachable!(),
					}))
				},
				Transform::Owned(o) => {
					Transform::Owned(o.map_failure(|(s, e)| match e {
						ApiError::JsonError(e) => (s, e),
						_ => unreachable!(),
					}))
				},
			};
			Json::from_data(request, outcome)
				.map(|v| Api {
					inner: v.into_inner(),
				})
				.map_failure(|(s, e)| (s, ApiError::JsonError(e)))
		} else {
			outcome.owned().map(|_| unreachable!())
		}
	}
}
