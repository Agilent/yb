use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tarpc::{client, context, tokio_serde::formats::Json};
use tarpc::client::RpcError;
use tarpc::context::Context;
use tokio::net::ToSocketAddrs;
use crate::error::ServiceResult;
use crate::service::ServiceClient;

pub struct Client {
    inner: ServiceClient,
}

impl Client {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> anyhow::Result<Self> {
        let transport = tarpc::serde_transport::tcp::connect(addr, Json::default).await?;
        let client = ServiceClient::new(client::Config::default(), transport).spawn();

        Ok(Self {
            inner: client
        })
    }

    pub fn lookup_or_clone<U: Into<String>>(&self, uri: U) -> impl futures::Future<Output = Result<ServiceResult<PathBuf>, RpcError>> + '_ {
        self.inner.lookup_or_clone(Self::make_context(), uri.into())
    }

    pub fn lookup<U: Into<String>>(&self, uri: U) -> impl futures::Future<Output = Result<Option<ServiceResult<PathBuf>>, RpcError>> + '_ {
        self.inner.lookup(Self::make_context(), uri.into())
    }

    pub fn clone_in<U: Into<String>, P: Into<PathBuf>>(&self, uri: U, parent_dir: P) -> impl futures::Future<Output = Result<ServiceResult<()>, RpcError>> + '_ {
        self.inner.clone_in(Self::make_context(), uri.into(), parent_dir.into())
    }

    fn make_context() -> Context {
        let mut context = context::current();
        context.deadline = SystemTime::now() + Duration::from_secs(60 * 5);
        context
    }
}
