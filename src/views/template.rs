#[macro_export]
macro_rules! template {
	($template_name:literal) => {
		{
			use askama::Template;
			#[derive(Template, Debug)]
			#[template(path = $template_name)]
			struct TemplateStruct {
				#[allow(dead_code)]
				zauth_version: &'static str
			}
			TemplateStruct {
				zauth_version: crate::ZAUTH_VERSION,
			}
		}
	};

	($template_name:literal; $($name:ident: $type:ty = $value:expr),+$(,)?) => {
		{
			use askama::Template;
			#[derive(Template, Debug)]
			#[template(path = $template_name)]
			struct TemplateStruct {
				#[allow(dead_code)]
				zauth_version: &'static str,
				$(
					$name: $type,
				)+
			}
			TemplateStruct {
				zauth_version: crate::ZAUTH_VERSION,
				$(
					$name: $value,
				)+
			}
		}
	}
}
