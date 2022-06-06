use rocket::data::{Data, FromData, Outcome};
use rocket::form::Form;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::Request;

use std::marker::PhantomData;

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

#[derive(Debug)]
pub struct SplitApi<FT, JT, RT> {
	inner:        RT,
	form_phantom: PhantomData<FT>,
	json_phantom: PhantomData<JT>,
}

impl<FT, JT, RT: std::fmt::Debug> SplitApi<FT, JT, RT> {
	pub fn into_inner(self) -> RT {
		self.inner
	}
}

#[derive(Debug)]
pub enum SplitApiError<'r, FT, JT>
where
	Form<FT>: FromData<'r>,
	Json<JT>: FromData<'r>,
	FT: std::fmt::Debug,
	JT: std::fmt::Debug,
{
	FormError(<Form<FT> as FromData<'r>>::Error),
	JsonError(<Json<JT> as FromData<'r>>::Error),
	WasNeither,
}

#[rocket::async_trait]
impl<'r, FT: 'r, JT: 'r, RT: 'r> FromData<'r> for SplitApi<FT, JT, RT>
where
	Form<FT>: FromData<'r>,
	Json<JT>: FromData<'r>,
	FT: Into<RT>,
	JT: Into<RT>,
	FT: std::fmt::Debug,
	JT: std::fmt::Debug,
	RT: std::fmt::Debug,
{
	type Error = SplitApiError<'r, FT, JT>;

	async fn from_data(
		request: &'r Request<'_>,
		data: Data<'r>,
	) -> Outcome<'r, Self> {
		if request.content_type() == Some(&ContentType::Form) {
			Form::from_data(request, data)
				.await
				.map(|v| SplitApi {
					inner:        v.into_inner().into(),
					form_phantom: PhantomData,
					json_phantom: PhantomData,
				})
				.map_failure(|(s, e)| (s, SplitApiError::FormError(e)))
		} else if request.content_type() == Some(&ContentType::JSON) {
			Json::from_data(request, data)
				.await
				.map(|v| SplitApi {
					inner:        v.into_inner().into(),
					form_phantom: PhantomData,
					json_phantom: PhantomData,
				})
				.map_failure(|(s, e)| (s, SplitApiError::JsonError(e)))
		} else {
			Outcome::Failure((Status::NotAcceptable, SplitApiError::WasNeither))
		}
	}
}
