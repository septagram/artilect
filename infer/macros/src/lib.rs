use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

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
