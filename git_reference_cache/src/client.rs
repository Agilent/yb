use futures::SinkExt;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::{Decoder, Framed, LinesCodec};

use crate::error::Result;

pub struct GitReferenceCacheClient {
    stream: Framed<TcpStream, LinesCodec>,
}

impl GitReferenceCacheClient {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;

        Ok(Self {
            stream: LinesCodec::new().framed(stream)
        })
    }

    pub async fn clone<S: Into<String>>(&mut self, remote: S) {
        self.stream.send(remote.into()).await.unwrap();
    }
}
