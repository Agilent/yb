use concurrent_git_pool::{Client, RpcError, ServiceError, ServiceResult};
use std::path::PathBuf;
use tokio::process::Command;

#[derive(Clone)]
pub struct PoolHelper {
    inner: Option<Client>,
}

impl PoolHelper {
    pub async fn connect_or_local() -> anyhow::Result<Self> {
        if let Ok(var) = std::env::var("CONCURRENT_GIT_POOL") {
            eprintln!("connecting to: {}", &var);
            return Ok(Self {
                inner: Some(Client::connect(var).await?),
            });
        }

        Ok(Self { inner: None })
    }

    pub async fn clone_in<U: Into<String>>(
        &self,
        uri: U,
        parent_dir: Option<PathBuf>,
        directory: Option<String>,
    ) -> Result<ServiceResult<()>, RpcError> {
        if let Some(inner) = &self.inner {
            let uri = uri.into();
            eprintln!("cloning: {}", &uri);
            let ret = inner.clone_in(uri, parent_dir, directory).await;
            dbg!(&ret);
            return ret;
        }

        let mut command = Command::new("git");
        command.env("GIT_TERMINAL_PROMPT", "0");
        command.arg("clone").arg(uri.into());
        if let Some(directory) = directory {
            command.arg(directory);
        }
        if let Some(parent_dir) = parent_dir {
            command.current_dir(parent_dir);
        }
        let result = command.output().await;

        if let Err(e) = result {
            return Ok(Err(ServiceError::CloneFailed(format!("{}", e))));
        }
        Ok(Ok(()))
    }
}
