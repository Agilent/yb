use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum ServiceError {
    #[error("The git clone operation failed: {}", .0)]
    CloneFailed(String),
    #[error("IO error encountered: {}", .0)]
    IoError(String),
}

impl From<io::Error> for ServiceError {
    fn from(e: io::Error) -> Self {
        Self::IoError(format!("{e}"))
    }
}

pub type ServiceResult<T> = Result<T, ServiceError>;
