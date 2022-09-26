use std::path::PathBuf;

#[tarpc::service]
pub trait Service {
    async fn lookup_or_clone(uri: String) -> PathBuf;
    async fn lookup(uri: String) -> Option<PathBuf>;
    async fn clone_in(uri: String, parent_dir: PathBuf);
}
