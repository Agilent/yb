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
        let yb_env = require_yb_env(config)?;

        for stream in yb_env.stream_db().streams() {
            println!("{}:", stream.1.name());

            if let Some(reason) = &stream.1.broken_reason() {
                println!("\tstream is broken: {reason:?}");
            } else {
                for spec in stream.1.specs() {
                    println!("\t{}", spec.0);
                }
            }
        }

        Ok(())
    }
}
