#[macro_export]
macro_rules! template {
	($template_name:literal) => {
		Template::render($template_name, ())
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
	}
}
