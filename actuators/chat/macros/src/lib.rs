extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Identifiable)]
pub fn derive_identifiable(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl Identifiable for #name {
            fn get_id(&self) -> uuid::Uuid {
                self.id
            }
        }
    };

    TokenStream::from(expanded)
}
