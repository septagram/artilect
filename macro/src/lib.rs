#![feature(if_let_guard)]

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse_macro_input, punctuated::Punctuated, DeriveInput, Meta};

// Infer

#[proc_macro_derive(FromLlmReply)]
pub fn derive_from_llm_reply(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl FromLlmReply for #name {
            fn from_reply(reply: &str) -> Result<Self, ParseError> {
                find_and_parse_json(JsonType::Object, reply)
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(FromLlmReplyArrayItem)]
pub fn derive_from_llm_reply_array_item(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl FromLlmReplyArrayItem for #name {}

        impl FromLlmReplyArray for Vec<#name> {
            type Item = #name;
        }

        impl FromLlmReplyArray for std::rc::Rc<[#name]> {
            type Item = #name;
        }

        impl FromLlmReplyArray for std::sync::Arc<[#name]> {
            type Item = #name;
        }
    };

    TokenStream::from(expanded)
}

// For DTO classes

#[proc_macro_derive(Identifiable)]
pub fn derive_identifiable(input: TokenStream) -> TokenStream {
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

#[derive(Debug)]
struct DtoFlags {
    pub db: bool,
    pub ui: bool,
    pub clone: bool,
    pub request: bool,
    pub response: bool,
}

impl Default for DtoFlags {
    fn default() -> Self {
        Self {
            db: false,
            ui: false,
            clone: false,
            request: false,
            response: false,
        }
    }
}

#[proc_macro_attribute]
pub fn dto(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute as a structured list of arguments using syn
    // let mut args = syn::parse_macro_input!(attr as syn::AttributeArgs).into_iter();
    let args = syn::parse_macro_input!(attr with Punctuated<syn::Ident, syn::Token![,]>::parse_separated_nonempty);
    let mut args = args.into_iter();

    // The first argument is the precept name (must be a single identifier)
    let precept_name = args.next().expect("Expected precept name as first argument");

    // The rest are flags
    let mut flags = DtoFlags::default();

    for flag in args {
        match flag.to_string().as_str() {
            "db" => flags.db = true,
            "ui" => flags.ui = true,
            "clone" => flags.clone = true,
            "request" => flags.request = true,
            "response" => flags.response = true,
            other => panic!("Unknown flag: {}", other),
        }
    }

    let mut universal_derives: Vec<syn::Path> = vec![syn::parse_quote!(Debug)];
    let mut item: syn::Item = syn::parse_macro_input!(item as syn::Item);
    let feature_in = format!("{}-in", precept_name);
    let feature_out = format!("{}-out", precept_name);
    let feature_front = format!("{}-front", precept_name);

    // Get a mutable reference to attrs for supported item types
    let (item_attrs, item_ident) = match &mut item {
        syn::Item::Struct(s) => (&mut s.attrs, &s.ident),
        syn::Item::Enum(e) => (&mut e.attrs, &e.ident),
        _ => panic!("dto macro only supports structs and enums"),
    };
    let actix_message_attr = item_attrs
        .iter()
        .position(|attr| attr.path().is_ident("actix_message"))
        .map(|index| item_attrs.remove(index));

    if flags.db {
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_in, derive(sqlx::FromRow))]
        });
    }

    if flags.ui {
        item_attrs.push(syn::parse_quote! {
            #[cfg_attr(feature = #feature_front, derive(PartialEq, Identifiable))]
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
            Meta::List(list) => Punctuated
                ::<syn::Ident, syn::Token![,]>
                ::parse_separated_nonempty
                .parse2(list.tokens)
                .expect(BASIC_SYNTAX_ERROR)
                .into_iter(),
            _ => panic!("{}", BASIC_SYNTAX_ERROR),
        };
        let response_type = actix_args
            .next()
            .expect(format!("Expected response_type as first argument. {}", BASIC_SYNTAX_ERROR).as_str());
        let message_type = actix_args.next();
        if actix_args.next().is_some() {
            panic!("Too many arguments - only response_type and optional message_type allowed. {}", BASIC_SYNTAX_ERROR);
        }

        let message_impl = quote! {
            #[cfg(feature = #feature_in)]
            impl actix::Message for crate::service::SignedMessage<#item_ident> {
                type Result = crate::service::Result<#response_type>;
            }
        };

        let message_definition = match message_type {
            Some(message_type) => quote! {
                #[cfg(feature = #feature_in)]
                pub type #message_type = crate::service::SignedMessage<#item_ident>;
            },
            None => quote! {}
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
    println!("{}", output);
    output
}

#[proc_macro_attribute]
pub fn message_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    let service_name = parse_macro_input!(attr as syn::Ident);
    let function = parse_macro_input!(item as syn::ItemFn);
    let fn_name = &function.sig.ident;

    // Get second argument type (the message type)
    let second_arg = function.sig.inputs.iter().nth(1)
        .expect("Function must have a second argument");
    let arg_type = match second_arg {
        syn::FnArg::Typed(pat_type) => &pat_type.ty,
        _ => panic!("Second argument must be typed"),
    };

    // Get return type
    let return_type = match &function.sig.output {
        syn::ReturnType::Type(_, ty) => ty,
        _ => panic!("Function must have a return type"),
    };

    let handler = quote! {
        #function

        impl Handler<#arg_type> for #service_name {
            type Result = actix::ResponseFuture<#return_type>;

            fn handle(&mut self, message: #arg_type, _: &mut Self::Context) -> Self::Result {
                let state = self.state.clone();
                Box::pin(async move {
                    #fn_name(&*state, message).await
                })
            }
        }
    };

    TokenStream::from(handler)
}
