use std::rc::Rc;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, parse_quote, ItemFn, Token};
use syn::parse::{Parse, ParseStream};

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

    let items = vec![
        parse_quote! {
            cfg_block::cfg_block! {
                #[artilect_macro::if_precept_in(#precept_name)] {
                    #(#items)*
                }
            }
        },
    ];

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
