use crate::pool::Pool;
use std::path::PathBuf;
use std::sync::Arc;
use crate::service::Service;
use tarpc::context::Context;

#[derive(Clone)]
pub struct Server {
    cache: Arc<Pool>,
}

#[tarpc::server]
impl Service for Server {
    async fn lookup_or_clone(self, _: Context, uri: String) -> PathBuf {
        self.cache.lookup_or_clone(uri).await.unwrap()
    }

    async fn lookup(self, _: Context, uri: String) -> Option<PathBuf> {
        None//self.cache.lookup_or_clone(uri).await.unwrap()
    }

    async fn clone_in(self, _: Context, uri: String, parent_dir: PathBuf) {
        //self.cache.lookup_or_clone(uri).await.unwrap()
    }
}

impl Server {
    pub fn new(cache: Arc<Pool>) -> Self {
        Self {
            cache,
        }
    }
}
