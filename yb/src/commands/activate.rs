use async_trait::async_trait;
use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::config::Config;
use crate::core::tool_context::require_yb_env;
use crate::errors::YbResult;
use crate::ui_ops::check_broken_streams::{ui_op_check_broken_streams, UiCheckBrokenStreamsOptions};
use crate::util::indicatif::MultiProgressHelpers;
use crate::yb_env::YbEnv;

/// Make the given spec active, but don't actually sync anything
#[derive(Debug, clap::Parser)]
pub struct ActivateCommand {
    /// Name of the spec to activate
    spec: String,
}

#[async_trait]
impl SubcommandRunner for ActivateCommand {
    async fn run(&self, config: &mut Config, mp: &MultiProgress) -> YbResult<()> {
        ui_op_check_broken_streams(UiCheckBrokenStreamsOptions::new(config, mp))?;

        let arena = toolshed::Arena::new();
        let mut yb_env = require_yb_env(config, &arena)?;

        if yb_env.stream_db().is_empty() {
            mp.warn("couldn't activate a spec because there are no streams");
            mp.warn("use 'yb stream add' first");
            panic!();
        }

        activate_spec(&mut yb_env, &self.spec)
    }
}

pub fn activate_spec(yb_env: &mut YbEnv, name: &str) -> YbResult<()> {
    let spec = yb_env.find_spec(name)?.cloned();
    if let Some(spec) = spec {
        // TODO don't clone
        yb_env.activate_spec(spec)?;
        println!("Activated spec '{}'", &name);
    } else {
        eyre::bail!("spec with name '{}' not found", &name);
    }

    Ok(())
}
