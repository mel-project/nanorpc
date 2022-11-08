use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, spanned::Spanned, ItemTrait, ReturnType, TraitItem, Type};

#[proc_macro_attribute]
/// This procedural macro should be put on top of a `async_trait` trait with name ending in `...Protocol`, defining all the function signatures in the RPC protocol. Given a trait of name `FooProtocol`, the macro
/// - automatically derives an `nanorpc::RpcService` implementation for `FooService`, a generated type that wraps around anything that implements `FooProtocol` --- these would be types that are server implementations of the protocol.
/// - automatically generates `FooClient`, a client-side struct that wraps a `nanorpc::RpcTransport` and has methods mirroring `FooProtocol`.
pub fn nanorpc_derive(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemTrait);
    let input_again = input.clone();
    let protocol_name = input.ident;
    if !protocol_name.to_string().ends_with("Protocol") {
        panic!("trait must end with the word \"Protocol\"")
    }
    let server_struct_name = syn::Ident::new(
        &format!(
            "{}Service",
            protocol_name.to_string().trim_end_matches("Protocol")
        ),
        protocol_name.span(),
    );
    let client_struct_name = syn::Ident::new(
        &format!(
            "{}Client",
            protocol_name.to_string().trim_end_matches("Protocol")
        ),
        protocol_name.span(),
    );
    let error_struct_name = syn::Ident::new(
        &format!(
            "{}Error",
            protocol_name.to_string().trim_end_matches("Protocol")
        ),
        protocol_name.span(),
    );

    // Generate the server implementation.
    let mut server_match = quote! {};
    let mut client_body = quote! {};
    for item in input.items {
        match item {
            TraitItem::Method(inner) => {
                let method_name = inner.sig.ident.clone();
                // create the block of code needed for calling the function
                // TODO check that it does in fact take "self"
                let mut offset = 0;
                let method_call = inner
                    .sig
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(idx, arg)| match arg {
                        syn::FnArg::Receiver(_) => {
                            offset += 1;
                            quote! {&self.0}
                        }
                        syn::FnArg::Typed(_) => {
                            let index = idx - offset;
                            quote! {if let ::std::option::Option::Some(::std::result::Result::Ok(v)) = __nrpc_args.get(#index).map(|v|::serde_json::from_value(v.clone())) {v} else {
                                // badly formatted argument
                                return Some(
                                    ::std::result::Result::Err(nanorpc::ServerError{
                                        code: 1,
                                        message: format!("deserialization of argument {} failed", #index),
                                        details: ::serde_json::Value::Null
                                    })
                                )
                            }}
                            // TODO handle this properly without a stupid clone
                        }
                    })
                    .reduce(|a, b| quote! {#a,#b})
                    .unwrap();
                // let method_call = method_call.to_string();
                let method_name_str = method_name.to_string();

                // TODO a better heuristic here
                let is_fallible = inner
                    .sig
                    .output
                    .to_token_stream()
                    .to_string()
                    .contains("Result");
                if is_fallible {
                    server_match = quote! {
                        #server_match
                        #method_name_str => {
                            let raw = #protocol_name::#method_name(#method_call).await;
                            let ok_mapped = raw.map(|o| ::serde_json::to_value(o).expect("serialization failed"));
                            let err_mapped = ok_mapped.map_err(|e| nanorpc::ServerError{
                                code: 1,
                                message: e.to_string(),
                                details: ::serde_json::to_value(e).expect("serialization failed")
                            });
                            ::std::option::Option::Some(err_mapped)
                        }
                    };
                } else {
                    server_match = quote! {
                        #server_match
                        #method_name_str => {
                            ::std::option::Option::Some(::std::result::Result::Ok(::serde_json::to_value(#protocol_name::#method_name(#method_call).await).expect("serialization failed")))
                        }
                    };
                }

                // Do the client
                let mut client_signature = inner.sig.clone();
                let original_output = match &client_signature.output {
                    ReturnType::Default => quote! {()},
                    ReturnType::Type(_, t) => t.to_token_stream(),
                };
                client_signature.output = ReturnType::Type(
                    syn::Token! [->](client_signature.span()),
                    Box::new(Type::Verbatim(
                        quote! {::std::result::Result<#original_output, #error_struct_name<__nrpc_T::Error>>},
                    )),
                );
                let vec_build = client_signature
                    .inputs
                    .iter()
                    .filter_map(|arg| match arg {
                        syn::FnArg::Receiver(_) => None,
                        syn::FnArg::Typed(t) => match t.pat.as_ref() {
                            syn::Pat::Ident(varname) => {
                                Some(quote! {__vb.push(::serde_json::to_value(&#varname).unwrap())})
                            }
                            v => panic!("wild {:?}", v.to_token_stream()),
                        },
                    })
                    .fold(
                        quote! {
                            let mut __vb: ::std::vec::Vec<::serde_json::Value> = ::std::vec::Vec::with_capacity(8);
                        },
                        |a, b| quote! {#a; #b},
                    );
                let method_name = client_signature.ident.to_string();
                let return_handler = if is_fallible {
                    quote! {
                        match jsval  {
                            Ok(jsval) => {
                                let retval = ::serde_json::from_value(jsval).map_err(#error_struct_name::FailedDecode)?;
                                Ok(Ok(retval))
                            }
                            Err(serverr) => {
                                Ok(Err(::serde_json::from_value(serverr.details).map_err(#error_struct_name::FailedDecode)?))
                            }
                        }
                    }
                } else {
                    quote! {
                        match jsval  {
                            Ok(jsval) => {
                                let retval: #original_output = ::serde_json::from_value(jsval).map_err(#error_struct_name::FailedDecode)?;
                                Ok(retval)
                            }
                            Err(serverr) => {
                                Err(#error_struct_name::ServerFail)
                            }
                        }
                    }
                };
                client_body = quote! {
                    #client_body

                    pub #client_signature {
                        #vec_build;
                        let result = nanorpc::RpcTransport::call(&self.0, #method_name, &__vb).await.map_err(#error_struct_name::Transport)?;
                        match result {
                            None => Err(#error_struct_name::NotFound),
                            Some(jsval) => {
                                #return_handler
                            }
                        }
                    }
                }
            }
            _ => {
                panic!("does not support things other than methods in the trait definition")
            }
        }
    }

    // Generate the client implementation
    let client_type_comment = format!("Automatically generated client type that communicates to servers implementing the [{protocol_name}] protocol. See the [{protocol_name}] trait for further documentation.");
    let client_impl = quote! {
        #[doc=#client_type_comment]
        pub struct #client_struct_name<T: nanorpc::RpcTransport>(pub T);

        impl <__nrpc_T: nanorpc::RpcTransport + Send + Sync + 'static> #client_struct_name<__nrpc_T> {
            #client_body
        }
    };

    let error_type_comment = format!("Automatically generated error type that {client_struct_name} instances return from its methods");
    let server_type_comment = format!("Automatically generated struct that wraps any 'business logic' struct implementing [{protocol_name}], and returns a JSON-RPC server implementing [nanorpc::RpcService]. See the [{protocol_name}] trait for further documentation.");
    let assembled = quote! {
        #input_again

        #[doc=#server_type_comment]
        pub struct #server_struct_name<T: #protocol_name>(pub T);

        #[::async_trait::async_trait]
        impl <__nrpc_T: #protocol_name + ::std::marker::Sync + ::std::marker::Send + 'static> nanorpc::RpcService for #server_struct_name<__nrpc_T> {
            async fn respond(&self, __nrpc_method: &str, __nrpc_args: Vec<::serde_json::Value>) -> Option<Result<::serde_json::Value, nanorpc::ServerError>> {
                match __nrpc_method {
                #server_match
                _ => {None}
                }
            }
        }

        #[derive(::thiserror::Error, Debug)]
        #[doc=#error_type_comment]
        pub enum #error_struct_name<T> {
            #[error("verb not found")]
            NotFound,
            #[error("unexpected server error on an infallible verb")]
            ServerFail,
            #[error("failed to decode JSON response: {0}")]
            FailedDecode(::serde_json::Error),
            #[error("transport-level error: {0}")]
            Transport(T)
        }

        #client_impl
    };
    assembled.into()
}
