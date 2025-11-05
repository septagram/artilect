#![feature(if_let_guard)]

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

mod dto;
mod orchestra;
mod precept;
mod util;

macro_rules! export_attribute {
    ($module:ident::$func:ident) => {
        #[proc_macro_attribute]
        pub fn $func(input: TokenStream, item: TokenStream) -> TokenStream {
            $module::$func(input, item)
        }
    };
}

macro_rules! export_derive {
    ($name:ident, $module:ident::$func:ident) => {
        #[proc_macro_derive($name)]
        pub fn $func(input: TokenStream) -> TokenStream {
            $module::$func(input)
        }
    };
}

macro_rules! export_macro {
    ($module:ident::$func:ident) => {
        #[proc_macro]
        pub fn $func(input: TokenStream) -> TokenStream {
            $module::$func(input)
        }
    };
}

export_attribute!(precept::if_precept);
export_attribute!(precept::if_precept_in);
export_attribute!(precept::if_precept_out);
export_attribute!(precept::if_precept_front);
export_attribute!(precept::precept);
export_attribute!(precept::precept_message);
export_macro!(precept::route_callback);
export_derive!(Identifiable, dto::derive_identifiable);
export_attribute!(dto::dto);
export_macro!(orchestra::orchestra_from_precepts);
export_macro!(orchestra::init_orchestra);

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
