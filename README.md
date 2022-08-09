# nanorpc: magic library for a JSON-RPC 2.0 subset (WIP)

`nanorpc` implements a subset of JSON-RPC 2.0, notably with the lack of no-response "notifications" and lack of negative-value error codes.

The most interesting part of `nanorpc` is that it contains a derive macro, `#[derive(RpcService)]`, that given a trait that describes the server-side behavior of an RPC service, derives both raw JSON-RPC handlers and a client implementation:

```rust
#[derive(RpcService)]
pub trait MathService {
    /// Adds two numbers
    fn add(&self, x: f64, y: f64) -> f64
    /// Multiplies two numbers
    fn mult(&self, x: f64, y: f64) -> f64
}

// Autogenerates a blanket RpcService impl:
impl <T: MathService> RpcService for T {...}

// Autogenerates a client struct like:

pub struct MathServiceClient<T: RpcTransport>(pub T):

impl <T: RpcTransport> MathServiceClient {
    /// Adds two numbers
    pub fn add(&self, x: f64, y: f64) -> Result<f64, T::Error>;
    ...
}
```

**Note**: right now the library is NOT done. Do not attempt to use it yet, the above is just a sketch of what it _will_ do.
