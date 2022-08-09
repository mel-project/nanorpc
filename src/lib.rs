use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A raw, JSON-RPC request ID. This should usually never be manually constructed.
pub enum JrpcId {
    Number(i64),
    String(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A raw, JSON-RPC request. This should usually never be manually constructed.
pub struct JrpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<serde_json::Value>,
    pub id: JrpcId,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// A raw, JSON-RPC response. This should usually never be manually constructed.
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
/// A raw, JSON-RPC error. This should usually never be manually constructed.
pub struct JrpcError {
    code: i64,
    message: String,
    data: serde_json::Value,
}

/// A server-returned error message. Contains a string description as well as a structured value.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerError {
    pub code: u32,
    pub message: String,
    pub details: serde_json::Value,
}

/// A trait that all nanorpc services implement. The only method that needs to be implemented is `respond`.
#[async_trait]
pub trait RpcService {
    /// Responds to an RPC call with method `str` and dynamically typed arguments `args`. The service should return `None` to indicate that this method does not exist at all. The internal error type
    async fn respond(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> Option<Result<serde_json::Value, ServerError>>;

    /// Responds to a raw JSON-RPC request, returning a raw JSON-RPC response.
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
        } else if let Some(response) = self.respond(&jrpc_req.method, &jrpc_req.params).await {
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

/// A client-side nanorpc transport. The only method that needs to be implemented is `call_raw`.
#[async_trait]
pub trait RpcTransport {
    /// This error type represents *transport-level* errors, like communication errors and such.
    type Error;
    /// Sends an RPC call to the remote side, returning the result. `Ok(None)` means that there is no transport-level error, but that the verb does not exist. This generally does not need a manual implementation.
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
            panic!("received malformed JrpcResponse from own call_raw")
        }
    }

    /// Sends an RPC call to the remote side, as a raw JSON-RPC request, receiving a raw JSON-RPC response.
    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error>;
}
