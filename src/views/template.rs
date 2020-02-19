#[macro_export]
macro_rules! template {
	($template_name:literal) => {
		{
			use askama::Template;
			#[derive(Template, Debug)]
			#[template(path = $template_name)]
			struct TemplateStruct {};
			TemplateStruct{}
		}
	};

	($template_name:literal; $($name:ident: $type:ty = $value:expr),+$(,)?) => {
		{
			use askama::Template;
			#[derive(Template, Debug)]
			#[template(path = $template_name)]
			struct TemplateStruct {
				$(
					$name: $type,
				)+
			}
			TemplateStruct {
				$(
					$name: $value,
				)+
			}
		}
	};
}
