use async_trait::async_trait;
use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::config::Config;
use crate::core::tool_context::require_yb_env;
use crate::errors::YbResult;

/// List the available specs
#[derive(Debug, clap::Parser)]
pub struct ListCommand {}

#[async_trait]
impl SubcommandRunner for ListCommand {
    async fn run(&self, config: &mut Config, _mp: &MultiProgress) -> YbResult<()> {
        let arena = toolshed::Arena::new();
        let yb_env = require_yb_env(config, &arena)?;
        for stream in yb_env.streams_by_name() {
            println!("{}:", stream.0);
            for spec in stream.1.specs_by_name() {
                println!("\t{}", spec.0);
            }
        }

        Ok(())
    }
}
