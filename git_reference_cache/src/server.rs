use crate::cache::ConcurrentGitCache;
use futures::{SinkExt, StreamExt};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_util::codec::{Decoder, LinesCodec};

use crate::error::{Result, Error};

pub async fn run_manager(mut rx: Receiver<Command>) {
    let cache = Arc::new(ConcurrentGitCache::new());

    while let Some(cmd) = rx.recv().await {
        let cache = cache.clone();
        tokio::spawn(async move {
            let p = cache.get_repo_for_remote(cmd.remote).await;
            println!("cloned! {:?}", p);
            cmd.resp.send(p).await.unwrap();
        });
    }
}

pub async fn run_server(listener: TcpListener, tx: Sender<Command>) -> io::Result<()> {
    loop {
        let (socket, _) = listener.accept().await?;

        let tx = tx.clone();

        // Spawn a new task for each connection
        tokio::spawn(async move {
            let mut framed = LinesCodec::new().framed(socket);
            let (resp_tx, mut resp_rx) = mpsc::channel(32);

            loop {
                tokio::select! {
                    Some(bytes) = framed.next() => {
                        if let Ok(bytes) = bytes {
                            println!("cloning: {}", &bytes);
                            let cmd = Command {
                                remote: bytes,
                                resp: resp_tx.clone()
                            };
                            tx.send(cmd).await.unwrap();
                            framed.send("Thanks, submitted :)").await.unwrap();
                        }
                    }
                    Some(resp) = resp_rx.recv() => {
                        match resp {
                            Ok(path) => {
                                framed.send(path.to_str().unwrap()).await.unwrap();
                            }
                            Err(e) => {
                                if let Error::CloneFailed(e) = e {
                                    framed.send(format!("sorry buddy, an error: {:?}", e.code())).await.unwrap();
                                } else {
                                    unimplemented!();
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}


#[derive(Debug)]
pub struct Command {
    remote: String,
    resp: Sender<Result<PathBuf>>,
}


