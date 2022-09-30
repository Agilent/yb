use async_trait::async_trait;
use self_update::{cargo_crate_version, Status};

use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::errors::YbResult;
use crate::util::indicatif::MultiProgressHelpers;
use crate::Config;

/// Automatically download the latest version of yb
#[derive(Debug, clap::Parser)]
pub struct SelfUpdateCommand {}

#[async_trait]
impl SubcommandRunner for SelfUpdateCommand {
    async fn run(&self, _config: &mut Config, mp: &MultiProgress) -> YbResult<()> {
        let status = self_update::backends::github::Update::configure()
            .repo_owner("Agilent")
            .repo_name("yb")
            .bin_name("yb")
            .show_download_progress(true)
            .current_version(cargo_crate_version!())
            .build()?
            .update()?;

        match status {
            Status::UpToDate(v) => mp.note(format!("Version {} is up-to-date!", v)),
            Status::Updated(v) => mp.note(format!("Updated to version {}", v)),
        }

        Ok(())
    }
}
