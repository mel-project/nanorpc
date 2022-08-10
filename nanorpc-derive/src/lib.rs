use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, ItemTrait, TraitItem};

#[proc_macro_attribute]
pub fn nanorpc(_: TokenStream, input: TokenStream) -> TokenStream {
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

    // Generate the body of the respond.
    let mut inner_match = quote! {};
    for item in input.items {
        match item {
            TraitItem::Method(inner) => {
                let method_name = inner.sig.ident;
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
                            quote! {::serde_json::from_value(__nrpc_args[#index].clone()).unwrap()}
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
                    inner_match = quote! {
                        #inner_match
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
                    inner_match = quote! {
                        #inner_match
                        #method_name_str => {
                            ::std::option::Option::Some(::std::result::Result::Ok(::serde_json::to_value(#protocol_name::#method_name(#method_call).await).expect("serialization failed")))
                        }
                    };
                }
            }
            _ => {
                panic!("does not support things other than methods in the trait definition")
            }
        }
    }

    let assembled = quote! {
        #input_again

        pub struct #server_struct_name<T: #protocol_name>(pub T);

        #[::async_trait::async_trait]
        impl <__nrpc_T: #protocol_name + ::std::marker::Sync + ::std::marker::Send + 'static> ::nanorpc::RpcService for #server_struct_name<__nrpc_T> {
            async fn respond(&self, __nrpc_method: &str, __nrpc_args: Vec<::serde_json::Value>) -> Option<Result<::serde_json::Value, ::nanorpc::ServerError>> {
                match __nrpc_method {
                #inner_match
                _ => {None}
                }
            }
        }
    };
    assembled.into()
}
