use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use indicatif::MultiProgress;

use crate::commands::activate::ActivateCommand;
use crate::commands::init::InitCommand;
use crate::commands::list::ListCommand;
use crate::commands::run::RunCommand;
use crate::commands::self_update::SelfUpdateCommand;
use crate::commands::status::*;
use crate::commands::stream::{
    StreamAddCommand, StreamListCommand, StreamSubcommands, StreamUpdateCommand,
};
use crate::commands::sync::SyncCommand;
use crate::commands::twice_bake::TwiceBakeCommand;
use crate::commands::upgrade::UpgradeCommand;
use crate::errors::YbResult;
use crate::Config;

mod activate;
mod init;
mod list;
mod run;
mod self_update;
pub mod status;
mod stream;
mod sync;
mod twice_bake;
mod upgrade;

#[async_trait]
#[enum_dispatch]
pub trait SubcommandRunner {
    async fn run(&self, config: &mut Config, mp: &MultiProgress) -> YbResult<()>;
}

#[enum_dispatch(SubcommandRunner)]
#[derive(Debug, clap::Parser)]
pub enum Subcommands {
    Init(InitCommand),
    Run(RunCommand),
    SelfUpdate(SelfUpdateCommand),
    Status(StatusCommand),
    #[clap(subcommand)]
    Stream(StreamSubcommands),
    Activate(ActivateCommand),
    Sync(SyncCommand),
    TwiceBake(TwiceBakeCommand),
    List(ListCommand),
    Upgrade(UpgradeCommand),
}
