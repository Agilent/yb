pub mod client;
mod error;
pub mod pool;
pub mod server;
pub mod service;
pub mod pool_helper;

pub use client::Client;
pub use error::{ServiceError, ServiceResult};

pub use pool_helper::PoolHelper;

pub use tarpc::client::RpcError;
