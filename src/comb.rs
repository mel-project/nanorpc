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
