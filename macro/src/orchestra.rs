use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

use crate::util::capitalize;

pub fn orchestra_from_precepts(input: TokenStream) -> TokenStream {
    let precepts =
        parse_macro_input!(input with Punctuated::<PreceptField, syn::Token![,]>::parse_terminated);
    let mut orchestra_fields = proc_macro2::TokenStream::new();
    let mut address_book_fields = proc_macro2::TokenStream::new();
    let mut address_book_converters = proc_macro2::TokenStream::new();
    let mut comparators = proc_macro2::TokenStream::new();

    for p in precepts.iter() {
        let precept = &p.name;
        let path = &p.path;
        let feature_in = format!("{}-in", precept);
        let feature_out = format!("{}-out", precept);
        let cfg_block = quote! {
            #[cfg(any(feature = #feature_in, feature = #feature_out))]
        };
        orchestra_fields.extend(cfg_block.clone());
        orchestra_fields.extend(quote! {
            pub #precept: crate::precepts::#path::Addr,
        });
        address_book_fields.extend(cfg_block.clone());
        address_book_fields.extend(quote! {
            pub #precept: crate::precepts::#path::Client,
        });
        address_book_converters.extend(cfg_block.clone());
        address_book_converters.extend(quote! {
            #precept: self.#precept.to_client(client_id.clone(), token.clone()),
        });
        comparators.extend(cfg_block);
        comparators.extend(quote! {
            if self.#precept != other.#precept { return false }
        });
    }

    let expanded = quote! {
        pub struct Orchestra {
            #orchestra_fields
        }

        impl Orchestra {
            pub fn to_address_book(&self, client_id: Option<crate::precept::Identity>, token: Option<std::sync::Arc<str>>) -> AddressBook {
                AddressBook {
                    #address_book_converters

                    client_id,
                    token,
                }
            }
        }

        pub struct AddressBook {
            client_id: Option<crate::precept::Identity>,
            token: Option<std::sync::Arc<str>>,

            #address_book_fields
        }
        
        impl PartialEq for Orchestra {
            fn eq(&self, other: &Self) -> bool {
                #comparators
                true
            }
        }
        
        impl PartialEq for AddressBook {
            fn eq(&self, other: &Self) -> bool {
                if self.client_id != other.client_id { return false }
                if self.token != other.token { return false }
                #comparators
                true
            }       
        }
    };

    expanded.into()
}

struct PreceptField {
    name: syn::Ident,
    path: syn::Path,
}

impl Parse for PreceptField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let path = input.parse()?;
        Ok(PreceptField { name, path })
    }
}
