use std::rc::Rc;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, parse_quote, ItemFn, Token};
use syn::parse::{Parse, ParseStream};

fn precept_conditional_compilation_attr(precept_name: &syn::Ident, feature_name: Option<&str>) -> syn::Attribute {
    match feature_name {
        Some(feature_name) => {
            let feature = format!("{}-{}", precept_name, feature_name);
            parse_quote! {
                #[cfg(feature = #feature)]
            }
        },
        None => {
            let feature_in = format!("{}-in", precept_name);
            let feature_out = format!("{}-out", precept_name);
            parse_quote! {
                #[cfg(any(feature = #feature_in, feature = #feature_out))]
            }
        },
    }
}

pub fn if_precept(input: TokenStream, item: TokenStream) -> TokenStream {
    let precept_name = parse_macro_input!(input as syn::Ident);
    let attr = precept_conditional_compilation_attr(&precept_name, None);
    let item = proc_macro2::TokenStream::from(item);
    quote! { #attr #item }.into()
}

pub fn if_precept_in(input: TokenStream, item: TokenStream) -> TokenStream {
    let precept_name = parse_macro_input!(input as syn::Ident);
    let attr = precept_conditional_compilation_attr(&precept_name, Some("in"));
    let item = proc_macro2::TokenStream::from(item);
    quote! { #attr #item }.into()
}

pub fn if_precept_out(input: TokenStream, item: TokenStream) -> TokenStream {
    let precept_name = parse_macro_input!(input as syn::Ident);
    let attr = precept_conditional_compilation_attr(&precept_name, Some("out"));
    let item = proc_macro2::TokenStream::from(item);
    quote! { #attr #item }.into()
}

pub fn if_precept_front(input: TokenStream, item: TokenStream) -> TokenStream {
    let precept_name = parse_macro_input!(input as syn::Ident);
    let attr = precept_conditional_compilation_attr(&precept_name, Some("front"));
    let item = proc_macro2::TokenStream::from(item);
    quote! { #attr #item }.into()
}

fn take_attribute(attr_name: &str, attrs: &mut Vec<syn::Attribute>) -> Option<syn::Attribute> {
    attrs
        .iter()
        .position(|attr| attr.path().is_ident(attr_name))
        .map(|pos| {
            attrs.remove(pos)
        })
}

pub fn precept(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(item as syn::ItemMod);
    let precept_name = if attr.is_empty() {
        module.ident.clone()
    } else {
        parse_macro_input!(attr as syn::Ident)
    };
    let (brace, items) = module.content.take().expect(
        "Precept module must have content. Consider using the `#![artilect_macro::precept]` macro as the first line of the precept module file."
    );
    let mut dto_module: Option<syn::ItemMod> = None;
    let mut front_module: Option<syn::ItemMod> = None;
    let mut message_handlers = Vec::new();
    let mut api_bindings = Vec::new();
    let mut items = items.into_iter().filter_map(|item| -> Option<syn::Item> {
        match item {
            syn::Item::Mod(module) if module.ident == "dto" => {
                let prev = dto_module.replace(module);
                if prev.is_some() {
                    panic!("Only one dto module is allowed per precept");
                };
                None
            },
            syn::Item::Mod(module) if module.ident == "front" => {
                let prev = front_module.replace(module);
                if prev.is_some() {
                    panic!("Only one front module is allowed per precept");
                };
                None
            },
            syn::Item::Struct(resources) if resources.ident == "Resources" => {
                // todo!()
                Some(resources.into())
            },
            syn::Item::Struct(state) if state.ident == "State" => {
                // todo!()
                Some(state.into())
            },
            syn::Item::Enum(agentic_state) if agentic_state.ident == "AgenticState" => {
                // todo!()
                Some(agentic_state.into())
            },
            syn::Item::Fn(mut message_handler_fn) => {
                if take_attribute("message_handler", &mut message_handler_fn.attrs).is_some() {
                    let message_handler = MessageHandler::from(&message_handler_fn);
                    message_handlers.push(message_handler.to_impl());
                    if let Some(attr) = take_attribute("api", &mut message_handler_fn.attrs) {
                        api_bindings.push(ApiBinding::new(message_handler, attr));
                    }
                };
                Some(message_handler_fn.into())
            },
            anything_else => Some(anything_else),
        }
    }).collect::<Vec<_>>();
    items.extend(message_handlers.into_iter().map(|handler| syn::Item::Impl(handler)));

    // Process API bindings into the router builder:
    if api_bindings.len() != 0 {
        items.extend(ApiBindings(api_bindings).into_routable());
    }

    let precept_in_attr = precept_conditional_compilation_attr(&precept_name, Some("in"));
    let mut items = vec![
        parse_quote! {
            cfg_block::cfg_block! {
                #precept_in_attr {
                    #(#items)*
                }
            }
        },
    ];

    if let Some(dto_module) = dto_module {
        items.push(dto_module.into());
    }

    if let Some(front_module) = front_module {
        let precept_front_attr = precept_conditional_compilation_attr(&precept_name, Some("front"));
        items.push(parse_quote! {
            #precept_front_attr
            #front_module
        });
    }

    module.content = Some((brace, items));
    module.attrs.insert(0, precept_conditional_compilation_attr(&precept_name, None));
    module.into_token_stream().into()
}

#[derive(Clone)]
struct MessageHandler {
    name: syn::Ident,
    input: Rc<syn::Type>,
    output: Rc<syn::Type>,
}

fn unpack_generic(ty: &syn::Type, expected_type: &str, checked_value: &str) -> Rc<syn::Type> {
    let type_path = match ty {
        syn::Type::Path(type_path)
        if type_path
            .path
            .segments
            .last()
            .map(|s| s.ident == expected_type)
            .unwrap_or(false)
        => type_path,
        _ => panic!("{} must be {}<T>", checked_value, expected_type),
    };
    let syn::PathArguments::AngleBracketed(args) =
        &type_path.path.segments.last().unwrap().arguments
    else {
        panic!("{} must have angle bracketed type parameters", expected_type);
    };
    match args.args.first() {
        Some(syn::GenericArgument::Type(inner_type)) => inner_type.clone().into(),
        _ => panic!("{} must have a type parameter", expected_type),
    }
}

impl From<&syn::ItemFn> for MessageHandler {
    fn from(function: &ItemFn) -> Self {
        let name = function.sig.ident.clone();

        // Get second argument type (the message type)
        let second_arg = function.sig.inputs.iter().nth(1)
            .expect("Function must have a second argument");
        let input = {
            let syn::FnArg::Typed(pat_type) = second_arg else {
                panic!("Second argument must be typed");
            };
            unpack_generic(&pat_type.ty, "SignedMessage", "Second argument")
        };

        // Get return type
        let output = match &function.sig.output {
            syn::ReturnType::Type(_, ty) => unpack_generic(ty, "Result", "Return type"),
            _ => panic!("Function must have a return type"),
        };

        Self { name, input, output }
    }
}

impl MessageHandler {
    fn to_impl(&self) -> syn::ItemImpl {
        let MessageHandler { name, input, output } = self;

        parse_quote! {
            impl actix::Handler<crate::precept::SignedMessage<#input>> for Precept {
                type Result = actix::ResponseFuture<crate::precept::Result<#output>>;

                fn handle(&mut self, message: crate::precept::SignedMessage<#input>, _: &mut Self::Context) -> Self::Result {
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
    pub fn to_extend_router_call(&self) -> proc_macro2::TokenStream {
        let Self { owner, args } = self;
        let MessageHandler { name, .. } = owner;
        let ApiBindingArgs { path, method, .. } = args;

        quote_spanned! { name.span() =>
            .route(#path, #method(axum_handlers::#name))
        }
    }

    pub fn into_handler_callback_impl(self) -> proc_macro2::TokenStream {
        let Self { owner, args } = self;
        let MessageHandler { name, input, output } = owner;
        let transformer = args.transformer.unwrap_or(parse_quote! { |Json(request): Json<#input>| request });
        let syn::ExprClosure { inputs, body, .. } = transformer;

        quote! {
            pub async fn #name(
                State(precept): State<actix::Addr<Precept>>,
                auth_header: TypedHeader<Authorization<Bearer>>,
                #inputs
            ) -> precept::Result<Json<#output>> {
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
                    .map_actix_error()
                    .map(|response| Json(response))
            }
        }
    }
}

struct ApiBindings(Vec<ApiBinding>);

impl ApiBindings {
    pub fn into_routable(self) -> [syn::Item; 2] {
        let api_binding_tokens = self.0
            .iter()
            .map(|binding| binding.to_extend_router_call());
        let routable_impl = parse_quote! {
            #[cfg(feature = "server-http2")]
            impl precept::Routable for actix::Addr<Precept> {
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

                    Router::new()#(#api_binding_tokens)*.with_state(self)
                }
            }
        };

        let handler_impls = self.0
            .into_iter()
            .map(|binding| binding.into_handler_callback_impl());

        [routable_impl, parse_quote! {
            #[cfg(feature = "server-http2")]
            mod axum_handlers {
                use super::*;
                use axum::{
                    routing::*,
                    extract::{Path, State},
                    Json,
                };
                use axum_extra::TypedHeader;
                use headers::authorization::{Authorization, Bearer};

                use crate::{
                    precept,
                    precept::{SignedMessage, Identity, ActixResult},
                };

                #(#handler_impls)*
            }
        }]
    }
}
