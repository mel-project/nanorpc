//! A server and client implementation for a "backdoor" protocol that allows clients to run arbitrary commands on the server.

use std::{net::SocketAddr, str::FromStr as _, sync::Arc};

mod protocol;
use clap::{Parser, Subcommand};
use nanorpc::{JrpcRequest, RpcService};
use protocol::*;
use warp::Filter;

/// Runs a server or client for the JSONRPC-over-HTTP-based backdoor protocol
#[derive(Parser, PartialEq, Debug)]
struct Args {
    #[command(subcommand)]
    nested: Subcommands,
}

#[derive(Subcommand, PartialEq, Debug)]
enum Subcommands {
    Server(ServerArgs),
    Client(ClientArgs),
}

/// Run a server.
#[derive(Parser, PartialEq, Debug)]
struct ServerArgs {
    /// Where to listen for HTTP requests
    #[arg(short, long, default_value_t = SocketAddr::from_str("0.0.0.0:11223").unwrap())]
    listen: SocketAddr,
}

/// Run a client.
#[derive(Parser, PartialEq, Debug)]
struct ClientArgs {
    /// Where to connect to
    #[arg(short, long, default_value_t = SocketAddr::from_str("127.0.0.1:11223").unwrap())]
    connect: SocketAddr,

    /// What to send
    #[arg(last = true)]
    commands: Vec<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
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
