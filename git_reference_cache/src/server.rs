use crate::cache::ConcurrentGitCache;
use std::path::PathBuf;
use std::sync::Arc;
use crate::service::Service;
use tarpc::context::Context;

#[derive(Clone)]
pub struct GitReferenceCacheServer {
    cache: Arc<ConcurrentGitCache>,
}

#[tarpc::server]
impl Service for GitReferenceCacheServer {
    async fn clone(self, _: Context, uri: String) -> PathBuf {
        self.cache.get_repo_for_remote(uri).await.unwrap()
    }
}

impl GitReferenceCacheServer {
    pub fn new(cache: Arc<ConcurrentGitCache>) -> Self {
        Self {
            cache,
        }
    }
}
