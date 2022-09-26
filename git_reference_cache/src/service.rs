use std::path::PathBuf;

#[tarpc::service]
pub trait Service {
    async fn clone(uri: String) -> PathBuf;
}
