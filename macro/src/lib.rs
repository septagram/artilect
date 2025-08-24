#![feature(if_let_guard)]

extern crate proc_macro;

use std::rc::Rc;
use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, parse_quote, punctuated::Punctuated, DeriveInput, ItemFn, Meta, Token};
use syn::parse::{Parse, Parser, ParseStream};

#[proc_macro_attribute]
pub fn if_precept(input: TokenStream, item: TokenStream) -> TokenStream {
    let precept_name = parse_macro_input!(input as syn::Ident);
    let feature_in = format!("{}-in", precept_name);
    let feature_out = format!("{}-out", precept_name);
    let item = proc_macro2::TokenStream::from(item);
    let expanded = quote! {
        #[cfg(any(feature = #feature_in, feature = #feature_out))]
        #item
    };
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn if_precept_in(input: TokenStream, item: TokenStream) -> TokenStream {
    let precept_name = parse_macro_input!(input as syn::Ident);
    let feature_in = format!("{}-in", precept_name);
    let item = proc_macro2::TokenStream::from(item);
    let expanded = quote! {
        #[cfg(feature = #feature_in)]
        #item
    };
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn if_precept_out(input: TokenStream, item: TokenStream) -> TokenStream {
    let precept_name = parse_macro_input!(input as syn::Ident);
    let feature_out = format!("{}-out", precept_name);
    let item = proc_macro2::TokenStream::from(item);
    let expanded = quote! {
        #[cfg(feature = #feature_out)]
        #item
    };
    TokenStream::from(expanded)
}

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
            impl actix::Message for crate::precept::SignedMessage<#item_ident> {
                type Result = crate::precept::Result<#response_type>;
            }
        };

        let message_definition = match message_type {
            Some(message_type) => quote! {
                #[cfg(feature = #feature_in)]
                pub type #message_type = crate::precept::SignedMessage<#item_ident>;
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
    output
}

fn take_attribute(attr_name: &str, attrs: &mut Vec<syn::Attribute>) -> Option<syn::Attribute> {
    attrs
        .iter()
        .position(|attr| attr.path().is_ident(attr_name))
        .map(|pos| {
            attrs.remove(pos)
        })
}

#[proc_macro_attribute]
pub fn precept(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(item as syn::ItemMod);
    let (brace, mut items) = module.content.take().expect("Precept module must have content. Consider using the `#![artilect_macro::precept]` macro as the first line of the precept module file.");
    let mut message_handlers = Vec::new();
    let mut api_bindings = Vec::new();
    for mut item in &mut items {
        match &mut item {
            syn::Item::Struct(resources) if resources.ident == "Resources" => {
                todo!()
            },
            syn::Item::Struct(state) if state.ident == "State" => {
                // todo!()
            },
            syn::Item::Enum(agentic_state) if agentic_state.ident == "AgenticState" => {
                todo!()
            },
            syn::Item::Fn(message_handler_fn) => {
                if take_attribute("message_handler", &mut message_handler_fn.attrs).is_some() {
                    let message_handler = MessageHandler::from(&*message_handler_fn);
                    message_handlers.push(message_handler.to_impl());
                    if let Some(attr) = take_attribute("api", &mut message_handler_fn.attrs) {
                        api_bindings.push(ApiBinding::new(message_handler, attr));
                    }
                }
            }
            _ => {},
        };
    };
    items.extend(message_handlers.into_iter().map(|handler| syn::Item::Impl(handler)));

    // Process API bindings into the router builder:
    if api_bindings.len() == 0 {
        items.push(ApiBindings(api_bindings).into_routable().into());
    }

    module.content = Some((brace, items));
    module.into_token_stream().into()
}

#[derive(Clone)]
struct MessageHandler {
    name: syn::Ident,
    input: Rc<syn::Type>,
    output: Rc<syn::Type>,
}

impl From<&syn::ItemFn> for MessageHandler {
    fn from(function: &ItemFn) -> Self {
        let name = function.sig.ident.clone();

        // Get second argument type (the message type)
        let second_arg = function.sig.inputs.iter().nth(1)
            .expect("Function must have a second argument");
        let input = match second_arg {
            syn::FnArg::Typed(pat_type) => pat_type.ty.clone().into(),
            _ => panic!("Second argument must be typed"),
        };

        // Get return type
        let output = match &function.sig.output {
            syn::ReturnType::Type(_, ty) => ty.clone().into(),
            _ => panic!("Function must have a return type"),
        };
        
        Self { name, input, output }
    }
}

impl MessageHandler {
    fn to_impl(&self) -> syn::ItemImpl {
        let MessageHandler { name, input, output } = self;
    
        parse_quote! {
            impl Handler<#input> for Precept {
                type Result = actix::ResponseFuture<#output>;
    
                fn handle(&mut self, message: #input, _: &mut Self::Context) -> Self::Result {
                    let state = self.state.clone();
                    Box::pin(async move {
                        #name(&*state, message).await
                    })
                }
            }
        }
    }
}

struct ApiBinding {
    owner: MessageHandler,
    args: ApiBindingArgs,
}

impl ApiBinding {
    pub fn new(owner: MessageHandler, input: syn::Attribute) -> Self {
        let args = input.parse_args().unwrap();
        ApiBinding { owner, args }
    }
}

struct ApiBindingArgs {
    path: syn::LitStr,
    method: syn::Ident,
    transformer: Option<syn::ExprClosure>,
}

impl Parse for ApiBindingArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;
        input.parse::<Token![,]>()?;
        let method = input.parse()?;
        let transformer = match input.parse::<Token![,]>() {
            Ok(_) => Some(input.parse()?),
            Err(_) => None,
        };
        Ok(ApiBindingArgs { path, method, transformer })
    }
}

impl ApiBinding {
    pub fn into_extend_router_call(self) -> proc_macro2::TokenStream {
        let Self { owner, args } = self;
        let MessageHandler { input, output, .. } = owner;
        let ApiBindingArgs { path, method, transformer } = args;
        let transformer = transformer.unwrap_or(parse_quote! { |value| value });
        let syn::ExprClosure { inputs, body, .. } = transformer;

        quote! {
            .route(#path, #method(
                |
                    State(precept): State<Addr<Precept>>,
                    auth_header: TypedHeader<Authorization<Bearer>>,
                    #inputs
                | -> precept::Result<Json<#output>> {
                    let user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| precept::Error::Unauthorized)?;
                    let data: #input = #body;
                    precept
                        .send(SignedMessage {
                            from: Identity {
                                user_id,
                                precept_id: None,
                            },
                            data,
                        })
                        .await
                        .into_precept_result()
                        .map(|response| Json(response))
                }
            ))
        }
    }
}

struct ApiBindings<I: IntoIterator<Item = ApiBinding>> (I);

impl<I: IntoIterator<Item = ApiBinding>> ApiBindings<I> {
    pub fn into_routable(self) -> syn::ItemImpl {
        let api_binding_tokens = self.0
            .into_iter()
            .map(|binding| binding.into_extend_router_call());
        parse_quote! {
            #[cfg(feature = "server-http2")]
            impl precept::Routable for Addr<Precept> {
                fn build_router(self) -> axum::Router {
                    use axum::{
                        routing::*,
                        extract::{Path, State},
                        Json,
                    };
                    use axum_extra::TypedHeader;
                    use headers::authorization::{Authorization, Bearer};

                    use crate::{
                        precept,
                        precept::{SignedMessage, Identity},
                    };

                    Router::new()#(#api_binding_tokens)*
                }
            }
        }
    } 
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[proc_macro]
pub fn orchestra_from_precepts(input: TokenStream) -> TokenStream {
    let precepts = parse_macro_input!(input with Punctuated::<PreceptField, syn::Token![,]>::parse_terminated);
    let mut orchestra_fields = proc_macro2::TokenStream::new();
    let mut address_book_fields = proc_macro2::TokenStream::new();
    let mut address_book_converters = proc_macro2::TokenStream::new();

    for p in precepts.iter() {
        let precept = &p.name;
        let path = &p.path;
        let feature_in = format!("{}-in", precept);
        let feature_out = format!("{}-out", precept);
        let cfg_block = quote! {
            #[cfg(any(feature = #feature_in, feature = #feature_out))]
        };
        let global_client_ident = format_ident!("Global{}Client", capitalize(precept.to_string().as_str()));
        let client_ident = format_ident!("{}Client", capitalize(precept.to_string().as_str()));
        orchestra_fields.extend(cfg_block.clone());
        orchestra_fields.extend(quote! {
            #precept: crate::precepts::#path::client::#global_client_ident,
        });
        address_book_fields.extend(cfg_block.clone());
        address_book_fields.extend(quote! {
            #precept: crate::precepts::#path::client::#client_ident,
        });
        address_book_converters.extend(cfg_block);
        address_book_converters.extend(quote! {
            #precept: self.#precept.to_client(client_id, token),
        });
    }

    let expanded = quote! {
        pub struct Orchestra {
            #orchestra_fields
        }

        impl Orchestra {
            pub fn to_address_book(&self, client_id: crate::precept::Identity, token: Option<std::sync::Arc<str>>) -> AddressBook {
                AddressBook {
                    #address_book_converters
                }
            }
        }

        pub struct AddressBook {
            #address_book_fields
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
