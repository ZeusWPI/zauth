#[macro_export]
macro_rules! template {
	($template_name:literal) => {
		{
			use rocket_contrib::templates::Template;
			#[derive(Serialize)]
			struct TemplateStruct {};
			Template::render($template_name, TemplateStruct{})
		}
	};

	($template_name:literal; $($name:ident: $type:ty = $value:expr),+$(,)?) => {
		{
			use rocket_contrib::templates::Template;
			#[derive(Serialize)]
			struct TemplateStruct {
				$(
					$name: $type,
				)+
			}
			Template::render(
				$template_name,
				TemplateStruct {
					$(
						$name: $value,
					)+
				}
			)
		}
	};
}
