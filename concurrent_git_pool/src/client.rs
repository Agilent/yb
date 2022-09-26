use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tarpc::{client, context, tokio_serde::formats::Json};
use tarpc::client::RpcError;
use tarpc::context::Context;
use tokio::net::ToSocketAddrs;
use crate::service::ServiceClient;

pub struct Client {
    inner: ServiceClient,
}

impl Client {
    pub async fn new<A: ToSocketAddrs>(addr: A) -> anyhow::Result<Self> {
        let transport = tarpc::serde_transport::tcp::connect(addr, Json::default).await?;
        let client = ServiceClient::new(client::Config::default(), transport).spawn();

        Ok(Self {
            inner: client
        })
    }

    pub fn lookup_or_clone(&self, uri: String) -> impl futures::Future<Output = Result<PathBuf, RpcError>> + '_ {
        self.inner.lookup_or_clone(Self::make_context(), uri)
    }

    pub fn lookup(&self, uri: String) -> impl futures::Future<Output = Result<Option<PathBuf>, RpcError>> + '_ {
        self.inner.lookup(Self::make_context(), uri)
    }

    pub fn clone_in(&self, uri: String, parent_dir: PathBuf) -> impl futures::Future<Output = Result<(), RpcError>> + '_ {
        self.inner.clone_in(Self::make_context(), uri, parent_dir)
    }

    fn make_context() -> Context {
        let mut context = context::current();
        context.deadline = SystemTime::now() + Duration::from_secs(60 * 5);
        context
    }
}
