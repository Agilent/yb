use crate::pool::Pool;
use std::path::PathBuf;
use std::sync::Arc;
use crate::service::Service;
use crate::error::ServiceResult;
use tarpc::context::Context;

#[derive(Clone)]
pub struct Server {
    cache: Arc<Pool>,
}

#[tarpc::server]
impl Service for Server {
    async fn lookup_or_clone(self, _: Context, uri: String) -> ServiceResult<PathBuf> {
        self.cache.lookup_or_clone(uri).await
    }

    async fn lookup(self, _: Context, uri: String) -> Option<ServiceResult<PathBuf>> {
        self.cache.lookup(uri).await
    }

    async fn clone_in(self, _: Context, uri: String, parent_dir: PathBuf) -> ServiceResult<()> {
        self.cache.clone_in(parent_dir, uri).await
    }
}

impl Server {
    pub fn new(cache: Arc<Pool>) -> Self {
        Self {
            cache,
        }
    }
}
