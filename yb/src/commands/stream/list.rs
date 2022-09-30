use async_trait::async_trait;
use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::core::tool_context::require_yb_env;
use crate::errors::YbResult;
use crate::util::paths::list_subdirectories_sorted;
use crate::Config;

#[derive(Debug, clap::Parser)]
pub struct StreamListCommand {}

#[async_trait]
impl SubcommandRunner for StreamListCommand {
    async fn run(&self, config: &mut Config, _mp: &MultiProgress) -> YbResult<()> {
        let arena = toolshed::Arena::new();
        let yb_env = require_yb_env(config, &arena)?;
        let streams_dir = yb_env.streams_dir();

        if streams_dir.exists() {
            println!();
            let streams = list_subdirectories_sorted(&streams_dir)?;
            for stream in streams {
                println!("{}", stream.file_name().unwrap().to_str().unwrap());
            }
        }

        Ok(())
    }
}
