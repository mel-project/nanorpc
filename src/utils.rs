use std::{future::Future, pin::Pin, sync::Arc};

use crate::{JrpcRequest, JrpcResponse, RpcService, RpcTransport, ServerError};

type DynRpcFuture = Pin<Box<dyn Future<Output = anyhow::Result<JrpcResponse>> + 'static>>;

/// A type-erased `RpcTransport` that uses `anyhow::Error` for transport errors.
///
/// This is convenient for hiding concrete transport error types behind a single
/// dynamic error, and avoids some trait object sharp edges.
pub struct DynRpcTransport {
    raw_caller: Box<dyn Fn(JrpcRequest) -> DynRpcFuture + Send + Sync + 'static>,
}

impl DynRpcTransport {
    /// Creates a new dynamically-typed transport from a concrete transport.
    pub fn new<T: RpcTransport>(t: T) -> Self
    where
        T::Error: Into<anyhow::Error>,
    {
        let t = Arc::new(t);
        Self {
            raw_caller: Box::new(move |req| {
                let t = t.clone();
                Box::pin(async move { t.call_raw(req).await.map_err(|e| e.into()) })
            }),
        }
    }
}

impl RpcTransport for DynRpcTransport {
    type Error = anyhow::Error;
    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        (self.raw_caller)(req).await
    }
}

/// An `RpcService` that tries one service and falls back to another.
///
/// If the first service returns `None` (method not found), the second service
/// gets a chance to respond.
pub struct OrService<T: RpcService, U: RpcService>(T, U);

impl<T: RpcService, U: RpcService> OrService<T, U> {
    /// Creates a new `OrService`.
    pub fn new(t: T, u: U) -> Self {
        Self(t, u)
    }
}

impl<T: RpcService, U: RpcService> RpcService for OrService<T, U> {
    async fn respond(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Option<Result<serde_json::Value, ServerError>> {
        if let Some(res) = self.0.respond(method, params.clone()).await {
            Some(res)
        } else {
            self.1.respond(method, params).await
        }
    }
}

/// An `RpcService` backed by an async function or closure.
///
/// This is useful for quick adapters or for testing without defining a new
/// struct type.
#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct FnService(
    Arc<
        dyn Fn(
                &str,
                Vec<serde_json::Value>,
            ) -> Pin<
                Box<
                    dyn std::future::Future<Output = Option<Result<serde_json::Value, ServerError>>>
                        + Send
                        + 'static,
                >,
            > + Sync
            + Send
            + 'static,
    >,
);

impl FnService {
    /// Creates a new function-backed service.
    pub fn new<
        Fut: std::future::Future<Output = Option<Result<serde_json::Value, ServerError>>>
            + Send
            + 'static,
        Fun: Fn(&str, Vec<serde_json::Value>) -> Fut + Send + Sync + 'static,
    >(
        f: Fun,
    ) -> Self {
        let f = Arc::new(f);
        Self(Arc::new(move |m, args| {
            let m = m.to_string();
            let f = f.clone();
            Box::pin(async move { f(&m, args).await })
        }))
    }
}

impl RpcService for FnService {
    async fn respond(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Option<Result<serde_json::Value, ServerError>> {
        self.0(method, params).await
    }
}
