use async_trait::async_trait;
use std::fmt::Debug;

pub(crate) use basic::*;
pub(crate) use bblayers::*;

use crate::errors::YbResult;
use crate::util::git::pool_helper::PoolHelper;

pub mod basic;
pub mod bblayers;

#[async_trait]
pub trait SyncAction: Debug + Send + Sync {
    fn is_force_required(&self) -> bool;
    async fn apply(&self, pool: &PoolHelper) -> YbResult<()>;
}
