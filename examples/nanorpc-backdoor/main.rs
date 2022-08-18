//! A server and client implementation for a "backdoor" protocol that allows clients to run arbitrary commands on the server.

use std::{net::SocketAddr, sync::Arc};

use argh::FromArgs;

mod protocol;
use nanorpc::{JrpcRequest, RpcService};
use protocol::*;
use warp::Filter;

/// Runs a server or client for the JSONRPC-over-HTTP-based backdoor protocol
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    #[argh(subcommand)]
    nested: Subcommands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Subcommands {
    Server(ServerArgs),
    Client(ClientArgs),
}

#[derive(FromArgs, PartialEq, Debug)]
/// Run a server.
#[argh(subcommand, name = "server")]
struct ServerArgs {
    /// where to listen for HTTP requests
    #[argh(option, default = "\"0.0.0.0:11223\".parse().unwrap()")]
    listen: SocketAddr,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Run a client.
#[argh(subcommand, name = "client")]
struct ClientArgs {
    /// where to connect to
    #[argh(option, default = "\"127.0.0.1:11223\".parse().unwrap()")]
    connect: SocketAddr,

    /// what to send
    #[argh(positional)]
    commands: Vec<String>,
}

#[tokio::main]
async fn main() {
    let args: Args = argh::from_env();
    match args.nested {
        Subcommands::Server(server) => {
            let service = Arc::new(BackdoorService(BackdoorImpl));
            let endpoint = warp::path("backdoor").and(warp::body::json()).and_then(
                move |item: JrpcRequest| {
                    let service = service.clone();
                    async move {
                        Ok::<_, warp::Rejection>(
                            serde_json::to_string(&service.respond_raw(item).await).unwrap(),
                        )
                    }
                },
            );
            warp::serve(endpoint).run(server.listen).await;
        }
        Subcommands::Client(cargs) => {
            let client = BackdoorClient(HttpTransport::new(format!(
                "http://{}/backdoor",
                cargs.connect
            )));
            let (scode, result) = client.system(cargs.commands.join(" ")).await.unwrap();
            eprintln!("status code: {}", scode);
            println!("{}", result);
            std::process::exit(scode)
        }
    }
}
