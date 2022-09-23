use std::io;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::mpsc;
use tracing::error;

use git_reference_cache::server::{run_server, run_manager};

#[tokio::main]
async fn main() -> io::Result<()> {
    // Bind the listener to the address
    let listener = TcpListener::bind("127.0.0.1:2345").await.unwrap();

    // Create a channel for the server and the manager to communicate over
    let (tx, mut rx) = mpsc::channel(32);

    tokio::select! {
        res = run_server(listener, tx) => {
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
        _ = run_manager(rx) => { /* todo error handling */ }
        _ = signal::ctrl_c() => {
            // The shutdown signal has been received.
            eprintln!("shutting down");
        }
    }

    Ok(())
}
