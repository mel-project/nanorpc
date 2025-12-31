#![allow(async_fn_in_trait)]
#[doc = include_str!("../README.md")]
mod utils;
pub use utils::*;

use std::sync::Arc;

pub use nanorpc_derive::nanorpc_derive;
#[doc(hidden)]
pub mod __macro_reexports {
    pub use anyhow;
    pub use serde_json;
    pub use thiserror;
}
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
/// A raw JSON-RPC request ID.
///
/// JSON-RPC allows numeric or string IDs. In most cases you should let
/// [`RpcTransport::call`] generate these for you.
pub enum JrpcId {
    Number(i64),
    String(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A raw JSON-RPC request.
///
/// Prefer `RpcTransport::call` when constructing requests from Rust types.
pub struct JrpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<serde_json::Value>,
    pub id: JrpcId,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A raw JSON-RPC response.
///
/// The JSON-RPC spec allows either `result` or `error` to be present.
/// In this crate, both may be `None` to represent a successful response
/// with a JSON `null` result.
pub struct JrpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub error: Option<JrpcError>,
    pub id: JrpcId,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A raw JSON-RPC error.
///
/// This mirrors the error object defined by the JSON-RPC 2.0 spec.
pub struct JrpcError {
    pub code: i64,
    pub message: String,
    pub data: serde_json::Value,
}

/// A server-returned error message.
///
/// When you implement [`RpcService::respond`], return `Err(ServerError { .. })`
/// to indicate that the method exists but failed to execute.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ServerError {
    pub code: u32,
    pub message: String,
    pub details: serde_json::Value,
}

/// Server-side RPC logic.
///
/// Implementors map a method name plus JSON values into either a JSON value
/// (success), a [`ServerError`] (method failed), or `None` (method not found).
/// In practice, you usually implement [`RpcService::respond`] directly and call
/// [`RpcService::respond_raw`] from a transport layer.
///
/// This trait is defined using Rust's native async trait support. [`RpcService`] has this definition:
///
/// ```
/// use nanorpc::{ServerError, JrpcRequest, JrpcResponse};
///
/// pub trait RpcService {
///     async fn respond(
///         &self,
///         method: &str,
///         params: Vec<serde_json::Value>,
///     ) -> Option<Result<serde_json::Value, ServerError>>;
///
///     async fn respond_raw(&self, jrpc_req: JrpcRequest) -> JrpcResponse;
/// }
/// ```
///
///
/// # Examples
///
/// ## Using an RpcService to respond to client requests
///
/// ```
/// use nanorpc::{RpcService, ServerError, JrpcRequest, JrpcResponse};
///
/// /// Object that implements the business logic
/// struct BusinessLogic;
///
/// impl RpcService for BusinessLogic {
///     async fn respond(&self,
///         method: &str,
///         params: Vec<serde_json::Value>
///     ) -> Option<Result<serde_json::Value, ServerError>> {
///         // business logic here
///         todo!()
///     }
/// }
///
/// /// Return the global BusinessLogic struct
/// fn bizlogic_singleton() -> &'static BusinessLogic { todo!() }
///
/// /// Handle a raw JSON-RPC request from, say, HTTP or TCP, returning the raw request
/// async fn handle_request(request: &[u8]) -> anyhow::Result<Vec<u8>> {
///     let request: JrpcRequest = serde_json::from_slice(request)?;
///     let response: JrpcResponse = bizlogic_singleton().respond_raw(request).await;
///     Ok(serde_json::to_vec(&response).unwrap())
/// }
pub trait RpcService: Sync + Send + 'static {
    /// Responds to an RPC call with method name and positional arguments.
    ///
    /// Return `None` to indicate the method does not exist. Returning
    /// `Some(Err(_))` indicates the method exists but failed at runtime.
    async fn respond(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Option<Result<serde_json::Value, ServerError>>;

    /// Responds to a raw JSON-RPC request, returning a raw JSON-RPC response.
    ///
    /// This default implementation handles version checks, method lookup,
    /// and error mapping.
    async fn respond_raw(&self, jrpc_req: JrpcRequest) -> JrpcResponse {
        if jrpc_req.jsonrpc != "2.0" {
            JrpcResponse {
                id: jrpc_req.id,
                jsonrpc: "2.0".into(),
                result: None,
                error: Some(JrpcError {
                    code: -32600,
                    message: "JSON-RPC version wrong".into(),
                    data: serde_json::Value::Null,
                }),
            }
        } else if let Some(response) = self.respond(&jrpc_req.method, jrpc_req.params).await {
            match response {
                Ok(response) => JrpcResponse {
                    id: jrpc_req.id,
                    jsonrpc: "2.0".into(),
                    result: Some(response),
                    error: None,
                },
                Err(err) => JrpcResponse {
                    id: jrpc_req.id,
                    jsonrpc: "2.0".into(),
                    result: None,
                    error: Some(JrpcError {
                        code: -1,
                        message: err.message,
                        data: err.details,
                    }),
                },
            }
        } else {
            JrpcResponse {
                id: jrpc_req.id,
                jsonrpc: "2.0".into(),
                result: None,
                error: Some(JrpcError {
                    code: -32601,
                    message: "Method not found".into(),
                    data: serde_json::Value::Null,
                }),
            }
        }
    }
}

impl<T: RpcService + ?Sized> RpcService for Arc<T> {
    async fn respond(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Option<Result<serde_json::Value, ServerError>> {
        self.as_ref().respond(method, params).await
    }
}

/// Client-side transport for sending JSON-RPC requests.
///
/// Implement [`RpcTransport::call_raw`] to define how raw JSON-RPC requests are
/// sent to the server (HTTP, TCP, in-process, etc.). Most callers should use
/// [`RpcTransport::call`], which handles request IDs and JSON mapping.
///
/// # Example
///
/// ```ignore
/// use nanorpc::RpcTransport;
///
/// let transport: impl RpcTransport = connect_to_server().await;
/// let three: u32 = serde_json::from_value(transport.call("add", &[1.into(), 2.into()]).await
///         .expect("transport failed")
///         .expect("no such verb")
///         .expect("server error"))
///     .expect("JSON decoding error");
/// assert_eq!(three, 3);
/// ```
pub trait RpcTransport: Sync + Send + 'static {
    /// This error type represents *transport-level* errors, like communication errors and such.
    type Error: Sync + Send + 'static;

    /// Sends an RPC call to the remote side, returning the result.
    ///
    /// `Ok(None)` means that there is no transport-level error, but the method
    /// does not exist. This generally does not need a manual implementation.
    async fn call(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> Result<Option<Result<serde_json::Value, ServerError>>, Self::Error> {
        let reqid = format!("req-{}", fastrand::u64(..));
        let req = JrpcRequest {
            jsonrpc: "2.0".into(),
            id: JrpcId::String(reqid),
            method: method.into(),
            params: params
                .iter()
                .map(|s| serde_json::to_value(s).unwrap())
                .collect(),
        };
        let result = self.call_raw(req).await?;
        if let Some(res) = result.result {
            Ok(Some(Ok(res)))
        } else if let Some(res) = result.error {
            if res.code == -32600 {
                Ok(None)
            } else {
                Ok(Some(Err(ServerError {
                    code: res.code as u32,
                    message: res.message,
                    details: res.data,
                })))
            }
        } else {
            // if both result and error are null, that means that the result is actually null and there is no error
            Ok(Some(Ok(serde_json::Value::Null)))
        }
    }

    /// Sends an RPC call to the remote side as a raw JSON-RPC request.
    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error>;
}

impl<T: RpcTransport + ?Sized> RpcTransport for Arc<T> {
    type Error = T::Error;

    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        self.as_ref().call_raw(req).await
    }
}

impl<T: RpcTransport + ?Sized> RpcTransport for Box<T> {
    type Error = T::Error;

    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        self.as_ref().call_raw(req).await
    }
}

// impl<T: RpcService + Sync> RpcTransport for T {
//     type Error = Infallible;

//     async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
//         Ok(self.respond_raw(req).await)
//     }
// }

#[cfg(test)]
mod tests {
    use crate::{self as nanorpc, ServerError};
    use nanorpc::{RpcService, nanorpc_derive};

    #[nanorpc_derive]
    pub trait MathProtocol {
        /// Adds two numbers
        async fn add(&self, x: f64, y: f64) -> f64;
        /// Multiplies two numbers
        async fn mult(&self, x: f64, y: f64) -> f64;
        /// Maybe fails
        async fn maybe_fail(&self) -> Result<f64, f64>;
    }

    struct Mather;

    impl MathProtocol for Mather {
        async fn add(&self, x: f64, y: f64) -> f64 {
            x + y
        }

        async fn mult(&self, x: f64, y: f64) -> f64 {
            x * y
        }

        async fn maybe_fail(&self) -> Result<f64, f64> {
            Err(12345.0)
        }
    }

    #[test]
    fn test_notfound_macro() {
        smol::future::block_on(async move {
            let service = MathService(Mather);
            assert_eq!(
                service
                    .respond("!nonexistent!", serde_json::from_str("[]").unwrap())
                    .await,
                None
            );
        });
    }

    #[test]
    fn test_simple_macro() {
        smol::future::block_on(async move {
            let service = MathService(Mather);
            assert_eq!(
                service
                    .respond("maybe_fail", serde_json::from_str("[]").unwrap())
                    .await
                    .unwrap()
                    .unwrap_err(),
                ServerError {
                    code: 1,
                    message: "12345".into(),
                    details: 12345.0f64.into()
                }
            );
            assert_eq!(
                service
                    .respond("add", serde_json::from_str("[1, 2]").unwrap())
                    .await
                    .unwrap()
                    .unwrap(),
                serde_json::Value::from(3.0f64)
            );
        });
    }
}
