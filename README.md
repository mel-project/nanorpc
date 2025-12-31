# nanorpc: typed, tiny JSON-RPC 2.0 (subset) for Rust

`nanorpc` lets you define an RPC protocol once (as a Rust trait) and get:

- A server wrapper that implements JSON-RPC request/response plumbing.
- A client wrapper with typed methods that call the server.
- A clean separation between business logic and transport.

The crate focuses on a small, practical subset of JSON-RPC 2.0: positional
arguments (`params` as an array) and request/response (not notifications).

## Quick start

Define a protocol with `#[nanorpc_derive]` and implement it as normal Rust:

```rust
use nanorpc::nanorpc_derive;

#[nanorpc_derive]
pub trait MathProtocol {
    async fn add(&self, x: f64, y: f64) -> f64;
    async fn mult(&self, x: f64, y: f64) -> f64;
    async fn maybe_fail(&self) -> Result<f64, String>;
}
```

The macro generates:

- `MathService<T: MathProtocol>` which implements `nanorpc::RpcService`.
- `MathClient<T: RpcTransport>` with methods `add`, `mult`, `maybe_fail`.
- `MathError<T>` which is the client-side error type.

## End-to-end example

This example uses an in-process loopback transport to show the entire flow
without any network code. A real transport would send JSON over HTTP, TCP, etc.

```rust,no_run
use nanorpc::{
    nanorpc_derive, JrpcRequest, JrpcResponse, RpcService, RpcTransport,
};

#[nanorpc_derive]
pub trait MathProtocol {
    async fn add(&self, x: f64, y: f64) -> f64;
    async fn maybe_fail(&self) -> Result<f64, String>;
}

struct MathImpl;

impl MathProtocol for MathImpl {
    async fn add(&self, x: f64, y: f64) -> f64 {
        x + y
    }

    async fn maybe_fail(&self) -> Result<f64, String> {
        Err("nope".into())
    }
}

struct Loopback<T>(T);

impl<T: RpcService> RpcTransport for Loopback<T> {
    type Error = std::convert::Infallible;

    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        Ok(self.0.respond_raw(req).await)
    }
}

async fn demo() {
    let service = MathService(MathImpl);
    let client = MathClient(Loopback(service));

    let sum = client.add(2.0, 3.0).await.unwrap();
    assert_eq!(sum, 5.0);

    let err = client.maybe_fail().await.unwrap_err();
    assert!(format!("{err}").contains("nope"));
}
```

## Error handling model

There are three layers of errors:

1. Transport errors: failures to send/receive a request (e.g., socket failure).
2. RPC-level errors: the method exists, but returns an error `Result`.
3. Not found: the method name does not exist on the server.

Generated client methods return `Result<T, ProtocolError<TransportErr>>`:

- `ProtocolError::Transport(e)` for transport failures.
- `ProtocolError::NotFound` if the server reports "method not found".
- `ProtocolError::FailedDecode` if a JSON decode fails.
- `ProtocolError::ServerFail` when an infallible method returns an error.

## Raw JSON-RPC layer

You can also build servers and clients directly on the JSON layer:

- `RpcService::respond_raw(JrpcRequest) -> JrpcResponse`
- `RpcTransport::call_raw(JrpcRequest) -> JrpcResponse`

The higher-level `respond`/`call` methods translate between JSON-RPC types and
`serde_json::Value`.

Example JSON-RPC request/response:

```json
{"jsonrpc": "2.0", "method": "mult", "params": [42, 23], "id": 1}
```

```json
{"jsonrpc": "2.0", "result": 966, "id": 1}
```

## Utilities

`nanorpc` ships a few helpers:

- `DynRpcTransport`: type-erased transport using `anyhow::Error`.
- `OrService`: chain two services; fall back to the second if the first
  does not recognize a method.
- `FnService`: wrap an async function/closure as a `RpcService`.

## Notes and limitations

- Parameters are positional (`Vec<serde_json::Value>`), not named.
- Requests are always JSON-RPC 2.0 and require an `id`.
- This is intentionally small; it does not implement notifications or batching.
