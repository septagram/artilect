use proc_macro::TokenStream;
use quote::quote;
use syn::{Meta, parse::Parser, parse_macro_input, punctuated::Punctuated};

use crate::util::take_attribute;

pub fn derive_identifiable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl crate::Identifiable for #name {
            fn get_id(&self) -> uuid::Uuid {
                self.id
            }
        }
    };

    TokenStream::from(expanded)
}

#[derive(Debug)]
struct DtoFlags {
    pub db: bool,
    pub eq: bool,
    pub ui: bool,
    pub clone: bool,
    pub request: bool,
    pub response: bool,
}

impl Default for DtoFlags {
    fn default() -> Self {
        Self {
            db: false,
            eq: false,
            ui: false,
            clone: false,
            request: false,
            response: false,
        }
    }
}

pub fn dto(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute as a structured list of arguments using syn
    // let mut args = syn::parse_macro_input!(attr as syn::AttributeArgs).into_iter();
    let args = syn::parse_macro_input!(attr with Punctuated<syn::Ident, syn::Token![,]>::parse_separated_nonempty);
    let mut args = args.into_iter();

    // The first argument is the precept name (must be a single identifier)
    let precept_name: syn::Ident = args
        .next()
        .expect("Expected precept name or 'always' as first argument");
    let precept_name = if precept_name == "always" { None } else { Some(precept_name) };

    // The rest are flags
    let mut flags = DtoFlags::default();

    for flag in args {
        match flag.to_string().as_str() {
            "db" => flags.db = true,
            "eq" => flags.eq = true,
            "ui" => flags.ui = true,
            "clone" => flags.clone = true,
            "request" => flags.request = true,
            "response" => flags.response = true,
            other => panic!("Unknown flag: {}", other),
        }
    }

    let mut universal_derives: Vec<syn::Path> = vec![syn::parse_quote!(Debug)];
    let mut item: syn::Item = syn::parse_macro_input!(item as syn::Item);
    let (feature_in, feature_out, feature_front) = match precept_name {
        Some(precept_name) => (
            format!("{}-in", precept_name),
            format!("{}-out", precept_name),
            format!("{}-front", precept_name),
        ),
        None => ("backend".into(), "client".into(), "frontend".into()),
    };

    // Get a mutable reference to attrs for supported item types
    let (item_attrs, item_ident) = match &mut item {
        syn::Item::Struct(s) => (&mut s.attrs, &s.ident),
        syn::Item::Enum(e) => (&mut e.attrs, &e.ident),
        _ => panic!("dto macro only supports structs and enums"),
    };
    let actix_message_attr = take_attribute("message", item_attrs);

    if flags.db {
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_in, derive(sqlx::FromRow))]
        });
    }

    if flags.eq || flags.ui {
        universal_derives.push(syn::parse_quote!(PartialEq));
    }

    if flags.ui {
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_front, derive(artilect_macro::Identifiable))]
        });
    }

    if flags.clone {
        universal_derives.push(syn::parse_quote!(Clone));
    }

    if flags.request && flags.response {
        universal_derives.push(syn::parse_quote!(Serialize));
        universal_derives.push(syn::parse_quote!(Deserialize));
    } else if flags.request {
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_in, derive(Deserialize))]
        });
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_out, derive(Serialize))]
        });
    } else if flags.response {
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_in, derive(Serialize))]
        });
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_out, derive(Deserialize))]
        });
    }

    if !universal_derives.is_empty() {
        item_attrs.push(syn::parse_quote! {
            #[derive(#(#universal_derives),*)]
        });
    }

    const BASIC_SYNTAX_ERROR: &str = "The actix_message attribute must have the following syntax: #[actix_message(ResponseType[, MessageType])].";
    let actix_message_output = if let Some(attr) = actix_message_attr {
        let mut actix_args = match attr.meta {
            Meta::List(list) => Punctuated::<syn::Ident, syn::Token![,]>::parse_separated_nonempty
                .parse2(list.tokens)
                .expect(BASIC_SYNTAX_ERROR)
                .into_iter(),
            _ => panic!("{}", BASIC_SYNTAX_ERROR),
        };
        let response_type = actix_args.next().expect(
            format!(
                "Expected response_type as first argument. {}",
                BASIC_SYNTAX_ERROR
            )
            .as_str(),
        );
        let message_type = actix_args.next();
        if actix_args.next().is_some() {
            panic!(
                "Too many arguments - only response_type and optional message_type allowed. {}",
                BASIC_SYNTAX_ERROR
            );
        }

        let message_impl = quote! {
            impl crate::precept::Message for #item_ident {
                type Response = #response_type;
            }
        };

        let message_definition = match message_type {
            Some(message_type) => quote! {
                #[cfg(feature = #feature_in)]
                pub type #message_type = crate::precept::SignedMessage<#item_ident>;
            },
            None => quote! {},
        };

        quote! {
            #message_impl
            #message_definition
        }
    } else {
        quote! {}
    };

    let output = TokenStream::from(quote! {
        #item
        #actix_message_output
    });
    output
}
