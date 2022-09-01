use std::fmt::Debug;

pub(crate) use basic::*;
pub(crate) use bblayers::*;

use crate::errors::YbResult;

pub mod basic;
pub mod bblayers;

pub trait SyncAction: Debug {
    fn is_force_required(&self) -> bool;
    fn apply(&self) -> YbResult<()>;
}
