use std::default::Default;

#[derive(Debug)]
pub struct CommonTemplateData {
	pub zauth_version: &'static str,
}

impl Default for CommonTemplateData {
	fn default() -> Self {
		CommonTemplateData {
			zauth_version: crate::ZAUTH_VERSION,
		}
	}
}

#[macro_export]
macro_rules! template {
	(
		$template_name:literal $(,{
			$($name:ident : $type:ty = $value:expr_2021),*
			$(,)?
		})?
	) => {
		{
			use askama::Template;

			use crate::errors::{InternalError,Result,ZauthError};
			use crate::views::template::CommonTemplateData;

			#[derive(Template)]
			#[template(path = $template_name)]
			struct TemplateStruct {
				#[allow(dead_code)]
				common: CommonTemplateData,
				$($($name: $type,)*)?
			}

			let instance = TemplateStruct {
				common: CommonTemplateData::default(),
				$($($name: $value,)*)?
			};

			let res: Result<String> = instance
				.render()
				.map_err(InternalError::from)
				.map_err(ZauthError::from);
			res
		}
	};
}
