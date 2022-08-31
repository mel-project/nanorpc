use std::{pin::Pin, sync::Arc};

use async_trait::async_trait;

use crate::{RpcService, ServerError};

/// An OrService responds to a call by trying one service then another.
pub struct OrService<T: RpcService, U: RpcService>(T, U);

impl<T: RpcService, U: RpcService> OrService<T, U> {
    /// Creates a new OrService.
    pub fn new(t: T, u: U) -> Self {
        Self(t, u)
    }
}

#[async_trait::async_trait]
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

/// A FnService wraps around a function that directly implements [Service::call_raw].
#[allow(clippy::type_complexity)]
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

#[async_trait]
impl RpcService for FnService {
    async fn respond(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Option<Result<serde_json::Value, ServerError>> {
        self.0(method, params).await
    }
}
