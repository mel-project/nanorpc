# nanorpc: magic library for a JSON-RPC 2.0 subset (WIP)

## Motivation

Typically, writing a client-server networked API (say, a REST API) involves the following three steps:

- Specify the protocol, in English
- Implement the server side of the protocol
- Separately implement the client side of the protocol

This is annoying and error-prone. The protocol is essentially specified three times in three different places and ways, and keeping them in sync is a huge chore.

Instead, we want to specify the protocol _once_, then automatically have:

- A server implementation, _generic_ over
  - The business logic of every endpoint
  - The low-level network details (e.g. "listen at this HTTP endpoint")
- A client implementation, generic over the low-level network details (e.g. "call this HTTP endpoint)
- Rust's type system fully utilized to help avoid bugs from things like serialization and deserialization mismatch, typos, etc.

## About `nanorpc`

`nanorpc` does exactly. It is a JSON-RPC subset implementation with a macro, `#[nanorpc_derive]`, that given a trait representing the API interface, abstracts away all the _duplicate_ parts of implementing an API.

In particular:

- `nanorpc` defines dynamically typed JSON-RPC server and client traits:
  - a trait `RpcService` that describes a JSON-RPC server-side responder (given a JSON request, produce a JSON response)
  - a trait `RpcTransport` that describes a JSON-RPC client-side requester (given a JSON request, talk to somebody else to produce a JSON response)
- `[nanorpc_derive]`, given a trait `FooProtocol` that describes the RPC methods, their argument types, and their return types, derives:
  - a struct `FooService` that, given any "business logic" struct that implements `FooProtocol`, wraps it into something implementing `RpcService`
  - a struct `FooClient` that, given any JSON transport implemetning `RpcTransport`, wraps it into a struct with methods corresponding to the RPC methods.

For example:

```rust
#[nanorpc_derive]
#[async_trait]
pub trait MathProtocol {
    /// Adds two numbers. Arguments and return type must be JSON-serializable through `serde_json`
    async fn add(&self, x: f64, y: f64) -> f64;
    /// Multiplies two numbers
    async fn mult(&self, x: f64, y: f64) -> f64;
}

// Autogenerates a server struct:
pub struct MathService<T: MathProtocol>(pub T);

#[async_trait]
impl <T: MathService> RpcService for MathService<T> {
    //...
}

// Autogenerates a client struct like:
pub struct MathClient<T: RpcTransport>(pub T);

impl <T: RpcTransport> MathClient {
    /// Adds two numbers
    pub async fn add(&self, x: f64, y: f64) -> Result<f64, T::Error>;

    //...
}
```

At the JSON level, the above protocol will respond to a JSON-RPC 2.0 request like:

```
{"jsonrpc": "2.0", "method": "mult", "params": [42, 23], "id": 1}
```

with

```
{"jsonrpc": "2.0", "result": 966, "id": 1}
```
