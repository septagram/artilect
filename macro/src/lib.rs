extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

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

#[proc_macro_derive(Authenticated)]
pub fn derive_authenticated(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl Authenticated for #name {
            fn get_from_user_id(&self) -> uuid::Uuid {
                self.from_user_id
            }

            fn set_from_user_id(&mut self, id: uuid::Uuid) {
                self.from_user_id = id;
            }
        }
    };

    TokenStream::from(expanded)
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
