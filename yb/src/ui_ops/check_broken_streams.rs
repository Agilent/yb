use crate::core::tool_context::maybe_yb_env;
use crate::errors::YbResult;
use crate::ops::update_stream::{op_update_stream, UpdateStreamOptions};
use crate::util::indicatif::MultiProgressHelpers;
use crate::yb_env::ActiveSpecStatus;
use crate::Config;
use dialoguer::Confirm;
use indicatif::MultiProgress;
use maplit::hashset;

#[derive(Debug)]
pub struct UiCheckBrokenStreamsOptions<'cfg> {
    config: &'cfg Config,
    mp: &'cfg MultiProgress,
    verbose: bool,
}

impl<'cfg> UiCheckBrokenStreamsOptions<'cfg> {
    pub fn new(config: &'cfg Config, mp: &'cfg MultiProgress) -> Self {
        Self {
            config,
            mp,
            verbose: false,
        }
    }

    pub fn verbose(&mut self, val: bool) -> &mut Self {
        self.verbose = val;
        self
    }
}

pub fn ui_op_check_broken_streams(options: UiCheckBrokenStreamsOptions) -> YbResult<()> {
    let arena = toolshed::Arena::new();

    let yb_env = match maybe_yb_env(options.config, &arena)? {
        Some(yb_env) => yb_env,
        None => {
            return Ok(());
        }
    };

    let active_spec_status = yb_env.active_spec_status();
    if let Some(ActiveSpecStatus::StreamsBroken(broken)) = &active_spec_status {
        options
            .mp
            .warn("one or more streams are broken, so the active spec could not be loaded");
        options.mp.note("error information follows below:");
        options.mp.suspend(|| eprintln!("{:?}", &broken));
        options.mp.println("")?;
        options
            .mp
            .note("would you like to try refresh the broken streams?");
        let confirm_result = options.mp.suspend(|| -> YbResult<bool> {
            Confirm::new()
                .with_prompt("Refresh streams?")
                .wait_for_newline(true)
                .interact()
                .map_err(|e| e.into())
        })?;

        if !confirm_result {
            options
                .mp
                .warn("OK, continuing with possibly limited functionality.");
            return Ok(());
        }

        let update_opts =
            UpdateStreamOptions::new(options.config, broken.keys().cloned().collect());
        op_update_stream(update_opts, |event| {})?;
    } else {
        return Ok(());
    }

    Ok(())
}
