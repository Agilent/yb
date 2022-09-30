use std::path::PathBuf;
use crate::error::ServiceResult;

#[tarpc::service]
pub trait Service {
    async fn lookup_or_clone(uri: String) -> ServiceResult<PathBuf>;
    async fn lookup(uri: String) -> Option<ServiceResult<PathBuf>>;
    async fn clone_in(uri: String, parent_dir: Option<PathBuf>, directory: Option<String>) -> ServiceResult<()>;
}
