pub mod client;
mod error;
pub mod pool;
pub mod server;
pub mod service;

pub use client::Client;
pub use error::{ServiceError, ServiceResult};

pub use tarpc::client::RpcError;
