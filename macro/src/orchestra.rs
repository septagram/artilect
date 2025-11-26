use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Token,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

use crate::util::DebugPrintCode;

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
            #precept: self.#precept.to_client(client_id.clone(), client.clone()),
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
            pub fn to_address_book(&self, client_id: Option<crate::precept::Identity>, client: Option<reqwest::Client>) -> AddressBook {
                AddressBook {
                    #address_book_converters

                    client_id,
                    client,
                }
            }
        }

        pub struct AddressBook {
            client_id: Option<crate::precept::Identity>,
            client: Option<reqwest::Client>,

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

pub fn orchestra(input: TokenStream) -> TokenStream {
    let precepts = parse_macro_input!(input with Punctuated::<OrchestraInitField, syn::Token![,]>::parse_terminated);
    let crate_ident = match std::env::var("CARGO_BIN_NAME") {
        Ok(_) => syn::Ident::new("artilect", proc_macro2::Span::call_site()),
        Err(_) => syn::Ident::new("crate", proc_macro2::Span::call_site()),
    };
    let mut address_preconstructors = proc_macro2::TokenStream::new();
    let mut orchestra_fields = proc_macro2::TokenStream::new();
    let mut precept_constructors = proc_macro2::TokenStream::new();
    let mut finalizers = quote! { orchestra };
    for field in precepts.into_iter() {
        match field {
            OrchestraInitField::Router {
                construct_router,
                return_expr,
            } => {
                if let Some(return_expr) = return_expr {
                    finalizers = quote! { #return_expr };
                }
                finalizers = quote! {
                    let router = #construct_router;
                    #finalizers
                };
            }
            OrchestraInitField::Precept {
                ident,
                construct_addr,
                construct_precept: Some(construct_precept),
            } => {
                let precept_path = &construct_precept.precept_path;
                let construct_config = construct_precept.construct_config(&crate_ident);
                let pending_ident = format_ident!("{}_pending", ident);
                let resolver_ident = format_ident!("{}_resolver", ident);
                let config_ident = format_ident!("{}_config", ident);
                let precept_id_ident = format_ident!("{}_precept_id", ident);
                let precept_identity_ident = format_ident!("{}_precept_identity", ident);
                address_preconstructors.extend(quote! {
                    let (#pending_ident, #resolver_ident) = #construct_addr;
                    let #config_ident = #construct_config;
                    let #precept_id_ident = #crate_ident::precepts::#precept_path::Precept::id(&#config_ident);
                    let #precept_identity_ident = #crate_ident::precept::Identity::Precept {
                        id: #precept_id_ident,
                        on_behalf_of: None,
                    };
                });
                orchestra_fields.extend(quote! {
                    #ident: #pending_ident,
                });
                precept_constructors.extend(quote! {
                    let #ident = #crate_ident::precepts::#precept_path::Precept::new(
                        orchestra.to_address_book(
                            Some(#precept_identity_ident),
                            Some(
                                #crate_ident::auth::middleware::make_access_token(
                                    #precept_identity_ident,
                                    #crate_ident::auth::middleware::AccessTokenType::Precept,
                                )
                                .expect("Failed to make access token for #ident precept.")
                                .token
                                .into(),
                            ),
                        ),
                        #config_ident,
                    ).start();
                    #resolver_ident.set(#ident.clone());
                });
            }
            OrchestraInitField::Precept {
                ident,
                construct_addr,
                construct_precept: None,
            } => orchestra_fields.extend(quote! {
                #ident: #construct_addr,
            }),
        }
    }
    quote! {{
        use actix::Actor;
        use #crate_ident::{
            precept::{Routable, PreceptConstructor, client::*},
            precepts::{
                cortex::auth::middleware::RouterAuth,
                cortex,
                vector,
            },
        };
        #address_preconstructors
        let orchestra = #crate_ident::orchestra::Orchestra {
            #orchestra_fields
        };
        #precept_constructors
        #finalizers
    }}
    .debug(None)
    .into()
}

enum OrchestraInitField {
    Precept {
        ident: syn::Ident,
        construct_addr: syn::Expr,
        construct_precept: Option<PreceptConstructInvocation>,
    },
    Router {
        construct_router: syn::Expr,
        return_expr: Option<syn::Expr>,
    },
}

struct PreceptConstructInvocation {
    pub precept_path: syn::Path,
    construct_config_source: syn::ExprStruct,
}

impl Parse for OrchestraInitField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        let is_router = name.to_string().as_str() == "router";
        input.parse::<Token![:]>()?;
        let main_expr: syn::Expr = input.parse()?;
        let has_secondary_initializer = input.parse::<Token![=>]>().is_ok();
        fn parse_secondary<T: Parse>(do_parse: bool, input: ParseStream) -> syn::Result<Option<T>> {
            if do_parse {
                Ok(Some(input.parse()?))
            } else {
                Ok(None)
            }
        }
        Ok(match is_router {
            false => Self::Precept {
                ident: name,
                construct_addr: main_expr,
                construct_precept: parse_secondary(has_secondary_initializer, input)?,
            },
            true => Self::Router {
                construct_router: main_expr,
                return_expr: parse_secondary(has_secondary_initializer, input)?,
            },
        })
    }
}

impl Parse for PreceptConstructInvocation {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut construct_config_source: syn::ExprStruct = input.parse()?;
        let precept_path = std::mem::replace(
            &mut construct_config_source.path,
            syn::Path {
                leading_colon: None,
                segments: Punctuated::new(),
            },
        );
        Ok(PreceptConstructInvocation {
            precept_path,
            construct_config_source,
        })
    }
}

impl PreceptConstructInvocation {
    pub fn construct_config(&self, crate_ident: &syn::Ident) -> syn::ExprStruct {
        let mut expr = self.construct_config_source.clone();
        let precept_path = &self.precept_path;

        // Build the qualified path properly
        expr.qself = Some(syn::QSelf {
            lt_token: Default::default(),
            ty: Box::new(parse_quote!(#crate_ident::precepts::#precept_path::Precept)),
            position: 1, // Position after PreceptConstructor
            as_token: Some(Default::default()),
            gt_token: Default::default(),
        });
        // I should go touch the grass

        expr.path = parse_quote!(PreceptConstructor::Config);

        expr
    }
}
