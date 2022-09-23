use std::io;
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("The git clone operation failed with code {:?}", .0.code())]
    CloneFailed(std::process::ExitStatus),
    #[error("IO error encountered")]
    IoError(Arc<io::Error>),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::IoError(Arc::new(e))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
