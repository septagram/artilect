use std::path::MAIN_SEPARATOR;

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{parse_macro_input, parse_quote, punctuated::Punctuated};

use crate::util::unpack_generic;

fn precept_conditional_compilation_attr(
    precept_name: &syn::Ident,
    feature_name: Option<&str>,
) -> syn::Attribute {
    match feature_name {
        Some(feature_name) => {
            let feature = format!("{}-{}", precept_name, feature_name);
            parse_quote! {
                #[cfg(feature = #feature)]
            }
        }
        None => {
            let feature_in = format!("{}-in", precept_name);
            let feature_out = format!("{}-out", precept_name);
            parse_quote! {
                #[cfg(any(feature = #feature_in, feature = #feature_out))]
            }
        }
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

fn current_file_path() -> Box<str> {
    let span = proc_macro2::Span::call_site();
    let local_file = span.local_file().unwrap();
    let mod_path = local_file.as_path();
    let mod_filename = mod_path.to_str().unwrap();
    Box::from(mod_filename)
}

fn get_precept_ident() -> syn::Ident {
    let current_file_path = current_file_path();
    let mut split_path: Vec<&str> = current_file_path.split(MAIN_SEPARATOR).collect();
    split_path.push(split_path.last().unwrap().split('.').next().unwrap());
    let pos = split_path
        .iter()
        .position(|cur| *cur == "local")
        .unwrap_or(0);
    if pos == 0 {
        panic!("Could not find precept name in file path");
    };
    let precept_name = split_path[pos - 1];
    syn::Ident::new(precept_name, proc_macro2::Span::call_site())
}

// Must be applied to both a precept module and a precept struct
pub fn precept(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as syn::Item);
    match item {
        syn::Item::Mod(module) => precept_mod(attr, module),
        syn::Item::Struct(struct_def) => precept_struct(attr, struct_def),
        _ => panic!("Precept macro accepts only modules and structs"),
    }
}

// - Add correct conditional compilation attributes onto a precept module and its submodules
pub fn precept_mod(_: TokenStream, mut module: syn::ItemMod) -> TokenStream {
    let precept_name = module.ident.clone();
    let (brace, mut items) = module.content.take().expect(
        "Precept module must have content. Consider using the `#![artilect_macro::precept]` macro as the first line of the precept module file."
    );
    for item in items.iter_mut() {
        match item {
            syn::Item::Mod(inner_module) => match inner_module.ident.to_string().as_str() {
                "local" => inner_module.attrs.insert(
                    0,
                    precept_conditional_compilation_attr(&precept_name, Some("in")),
                ),
                "remote" => inner_module.attrs.insert(
                    0,
                    precept_conditional_compilation_attr(&precept_name, Some("out")),
                ),
                "front" => inner_module.attrs.insert(
                    0,
                    precept_conditional_compilation_attr(&precept_name, Some("front")),
                ),
                _ => {}
            },
            _ => {}
        }
    }
    let feature_in = format!("{}-in", precept_name);
    let feature_out = format!("{}-out", precept_name);
    items.push(parse_quote! {
        cfg_block::cfg_block! {
            #[cfg(all(feature = #feature_in, not(feature = #feature_out)))] {
                pub type Addr = crate::precept::client::AddrLocal<local::Precept>;
                pub type Client = crate::precept::client::ClientLocal<local::Precept>;
            }

            #[cfg(all(not(feature = #feature_in), feature = #feature_out))] {
                pub type Addr = crate::precept::client::AddrRemote;
                pub type Client = crate::precept::client::ClientRemote;
            }

            #[cfg(all(feature = #feature_in, feature = #feature_out))] {
                pub type Addr = crate::precept::client::Addr<local::Precept>;
                pub type Client = crate::precept::client::Client<local::Precept>;
            }
        }
    });
    module.content = Some((brace, items));
    module
        .attrs
        .insert(0, precept_conditional_compilation_attr(&precept_name, None));
    module.into_token_stream().into()
}

// - Combine all the Add message routes into the precept router
// - Implement Precept trait
pub fn precept_struct(attr: TokenStream, struct_def: syn::ItemStruct) -> TokenStream {
    let struct_name = struct_def.ident.clone();
    let message_types =
        parse_macro_input!(attr with Punctuated<syn::Ident, syn::Token![,]>::parse_terminated);
    println!("ok");
    let message_type_iter = message_types.iter();
    let mut resources_type = None;
    for field in struct_def.fields.iter() {
        match field.ident.as_ref().unwrap().to_string().as_str() {
            "resources" => {
                resources_type = Some(unpack_generic(&field.ty, "Arc", "resources"));
            }
            _ => {}
        }
    }
    let resources_type =
        resources_type.expect("Precept struct must have a field named `resources`");
    quote! {
        #struct_def

        impl crate::precept::Precept for #struct_name {
            type Resources = #resources_type;
        }

        impl actix::Supervised for #struct_name {}

        #[cfg(feature = "server-http2")]
        impl crate::precept::Routable for actix::Addr<#struct_name> {
            fn build_router(self) -> axum::Router {
                let mut router = axum::Router::new();
                #(router = #message_type_iter::route(router);)*
                router.with_state(self)
            }
        }

        impl <M> actix::Handler<SignedMessage<M>> for #struct_name
        where
            M: crate::precept::MessageLocalStrategy<#struct_name>,
        {
            type Result = actix::ResponseFuture<crate::precept::Result<M::Response>>;

            fn handle(&mut self, message: crate::precept::SignedMessage<M>, _: &mut Self::Context) -> Self::Result {
                let resources = self.resources.clone();
                Box::pin(async move {
                    M::handle(&*resources, message.from, message.data).await
                })
            }
        }
    }.into()
}

pub fn precept_message(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(item as syn::ItemImpl);
    for item in item.items.iter_mut() {
        match item {
            syn::ImplItem::Fn(fn_impl) => match fn_impl.sig.ident.to_string().as_str() {
                "route" => fn_impl
                    .attrs
                    .insert(0, parse_quote! { #[cfg(feature = "server-http2")] }),
                _ => {}
            },
            _ => {}
        }
    }
    item.into_token_stream().into()
}

pub fn route_callback(item: TokenStream) -> TokenStream {
    let transformer = if !item.is_empty() {
        parse_macro_input!(item as syn::ExprClosure)
    } else {
        parse_quote! { |axum::Json(request): axum::Json<Self>| request }
    };
    let syn::ExprClosure { inputs, body, .. } = transformer;

    quote! {
        async |
            axum::extract::State(precept): axum::extract::State<actix::Addr<Precept>>,
            auth_header: axum_extra::TypedHeader<
                headers::authorization::Authorization<
                    headers::authorization::Bearer
                >
            >,
            #inputs
        | -> precept::Result<axum::Json<Self::Response>> {
            use crate::precept::ActixResult;
            let user_id = Uuid::parse_str(&auth_header.token()).map_err(|_| precept::Error::Unauthorized)?;
            let data: Self = #body;
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
                .map(|response| axum::Json(response))
        }
    }.into()
}
