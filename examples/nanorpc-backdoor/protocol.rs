use nanorpc::{nanorpc_derive, JrpcRequest, JrpcResponse, RpcTransport};
use tokio::process::Command;

/// The definition of the backdoor protocol.
#[nanorpc_derive]
pub trait BackdoorProtocol {
    /// Runs a command on the shell, returning the response code and stdout output.
    async fn system(&self, s: String) -> (i32, String);
}

/// Server implementation
pub struct BackdoorImpl;

impl BackdoorProtocol for BackdoorImpl {
    async fn system(&self, s: String) -> (i32, String) {
        eprintln!("running command {:?}", s);
        let output = Command::new("sh")
            .arg("-c")
            .arg(s)
            .output()
            .await
            .expect("cannot run command");
        (
            output.status.code().unwrap_or_default(),
            String::from_utf8_lossy(&output.stdout).into(),
        )
    }
}

/// Transport implementation
pub struct HttpTransport {
    client: reqwest::Client,
    url: String,
}

impl HttpTransport {
    pub fn new(url: String) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }
}

impl RpcTransport for HttpTransport {
    type Error = anyhow::Error;

    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        Ok(self
            .client
            .post(&self.url)
            .body(serde_json::to_string(&req)?)
            .send()
            .await?
            .json()
            .await?)
    }
}
