use nanorpc::{nanorpc, RpcService};

#[nanorpc]
#[async_trait::async_trait]
pub trait MathProtocol {
    /// Adds two numbers
    async fn add(&self, x: f64, y: f64) -> f64;
    /// Multiplies two numbers
    async fn mult(&self, x: f64, y: f64) -> f64;
    /// Maybe fails
    async fn maybe_fail(&self) -> Result<f64, f64>;
}

struct Mather;

#[async_trait::async_trait]
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

fn main() {
    smol::future::block_on(async move {
        let service = MathService(Mather);
        let client = MathClient(service);
        dbg!(client.add(1.0, 2.0).await.unwrap());
    });
}
