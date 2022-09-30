pub mod pool;
pub mod client;
pub mod server;
mod error;
pub mod service;

pub use client::Client;
pub use error::{ServiceResult, ServiceError};

pub use tarpc::client::RpcError;
