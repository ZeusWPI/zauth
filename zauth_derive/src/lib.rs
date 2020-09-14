use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics, Index};
use proc_macro2::{Span, Ident, Punct, Literal};
use std::collections::BTreeMap;
use proc_macro::TokenStream;

extern crate quote;
extern crate syn;

#[proc_macro_derive(Record)]
pub fn derive_record(input: TokenStream) -> TokenStream {

	// Parse the input tokens into a syntax tree.
	let input = parse_macro_input!(input as DeriveInput);
	let attributes = get_attributes(&input);
	dbg!(&input);

	//let table_name_string: &String = attributes.get("table_name").unwrap().first().unwrap();
	let table_name_string = dbg!(attributes.get("table_name").unwrap());
	let table: Ident = Ident::new(table_name_string, Span::call_site());

	//let database_type_string: &String = attributes.get("database").unwrap().first().unwrap();
	let database_type_string = dbg!(attributes.get("connection").unwrap());
	let db_ty = Ident::new(database_type_string, Span::call_site());

	// Get data from syntax tree
	let record_name = input.ident;
	let generics = add_trait_bounds(input.generics);
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

	let expanded = quote! {
        // The generated impl.
        impl #impl_generics ::zauth_record::Record<#db_ty> for #record_name #ty_generics #where_clause {
			type Output = #record_name;
			type Db = #db_ty;

			fn last(db: &#db_ty) -> Result<Self, ::diesel::errors:Error> {
				#table::table
					.order(#table::id.desc())
					.first(db)
			}
        }
    };

	dbg!(expanded.to_string());

	// Hand the output tokens back to the compiler.
	proc_macro::TokenStream::from(expanded)
}

// Add a bound `T: HeapSize` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
	for param in &mut generics.params {
		if let GenericParam::Type(ref mut type_param) = *param {
			type_param.bounds.push(parse_quote!(heapsize::HeapSize));
		}
	}
	generics
}

fn get_attributes(input: &DeriveInput) -> BTreeMap<String, String> {
	input.attrs.iter().filter_map(|attr| {
		let key = attr.path.segments.first()?.ident.to_string().clone();
		let mut tokens = attr.tokens.clone().into_iter();
		let _punc = tokens.next()?;
		let string_literal = tokens.next()?.to_string();
		let value = string_literal[1..(string_literal.len() - 1)].to_string();
		Some((key, value))
	}).collect()
}
