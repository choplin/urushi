//! Derive macros for urushi component data.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input, parse_quote};

/// Derives urushi's canonical row formatter for a struct.
#[proc_macro_derive(TableRow, attributes(table))]
pub fn derive_table_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_table_row(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_table_row(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = input.ident;
    let mut generics = input.generics;
    let fields = match input.data {
        Data::Struct(data) => data.fields,
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "TableRow can only be derived for structs",
            ));
        }
    };

    let mut writes = Vec::new();
    let mut displayed_types = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let mut skip = false;
        for attribute in &field.attrs {
            if !attribute.path().is_ident("table") {
                continue;
            }
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                    Ok(())
                } else {
                    Err(meta.error("only #[table(skip)] is supported"))
                }
            })?;
        }
        if skip {
            continue;
        }
        let member = field
            .ident
            .clone()
            .map(syn::Member::Named)
            .unwrap_or_else(|| syn::Member::Unnamed(index.into()));
        displayed_types.push(field.ty.clone());
        writes.push(quote! { cells.display(&self.#member); });
    }

    for ty in displayed_types {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: ::core::fmt::Display));
    }
    let urushi = match crate_name("urushi") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let name = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#name)
        }
        Err(error) => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("could not locate the urushi crate: {error}"),
            ));
        }
    };
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #urushi::TableRow for #name #type_generics #where_clause {
            fn write_cells(&self, cells: &mut #urushi::TableRowCells) {
                #(#writes)*
            }
        }
    })
}
