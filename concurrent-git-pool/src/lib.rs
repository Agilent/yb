pub mod client;
mod error;
pub mod pool;
pub mod pool_helper;
pub mod server;
pub mod service;

pub use client::Client;
pub use error::{ServiceError, ServiceResult};

pub use pool_helper::PoolHelper;

pub use tarpc::client::RpcError;
