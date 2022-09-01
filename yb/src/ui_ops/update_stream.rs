use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use std::time::Duration;

use crate::config::Config;
use crate::core::tool_context::maybe_yb_env;
use crate::errors::YbResult;
use crate::ops::update_stream::{op_update_stream, UpdateStreamEvent, UpdateStreamOptions};
use crate::util::indicatif::{IndicatifHelpers, MultiProgressHelpers};

use crate::yb_env::ConfigActiveSpecStatus;

#[derive(Debug)]
pub struct UiUpdateStreamOptions<'cfg> {
    config: &'cfg Config,
    mp: &'cfg MultiProgress,
    verbose: bool,
    fail_if_no_yb_env: bool,
}

impl<'cfg> UiUpdateStreamOptions<'cfg> {
    pub fn new(config: &'cfg Config, mp: &'cfg MultiProgress) -> Self {
        Self {
            config,
            mp,
            verbose: false,
            fail_if_no_yb_env: false,
        }
    }

    pub fn fail_if_no_yb_env(&mut self, val: bool) -> &mut Self {
        self.fail_if_no_yb_env = val;
        self
    }

    pub fn verbose(&mut self, val: bool) -> &mut Self {
        self.verbose = val;
        self
    }
}

pub fn ui_op_update_stream(options: UiUpdateStreamOptions) -> YbResult<()> {
    let arena = toolshed::Arena::new();

    let yb_env = match maybe_yb_env(options.config, &arena)? {
        Some(yb_env) => yb_env,
        None => {
            if options.fail_if_no_yb_env {
                eyre::bail!("expected yb environment; see the 'yb init' command")
            } else {
                return Ok(());
            }
        }
    };

    let active_spec_status = yb_env.active_spec_status();
    match &active_spec_status {
        ConfigActiveSpecStatus::ActiveSpec { name } => {
            options.mp.note(&format!("active spec: {}", name))
        }
        ConfigActiveSpecStatus::NoActiveSpec => options
            .mp
            .note("no active spec; consider using the 'yb activate' command"),
        ConfigActiveSpecStatus::NoYbEnv => options
            .mp
            .warn("consider using the 'yb init' command to create a yb environment"),
    }

    let update_opts = UpdateStreamOptions::new(options.config);

    // TODO report result in porcelain

    let mut stream_update_spinner: Option<ProgressBar> = None;
    op_update_stream(update_opts, |event| match event {
        UpdateStreamEvent::Start => {
            stream_update_spinner.replace(
                options.mp.add(
                    ProgressBar::new_spinner()
                        .with_message("refreshing stream")
                        .with_steady_tick(Duration::from_millis(50)),
                ),
            );
        }
        UpdateStreamEvent::ActiveStreamUpdated => {
            stream_update_spinner
                .as_ref()
                .unwrap()
                .set_style(ProgressStyle::with_template("{msg}: stream updated").unwrap());
        }
        UpdateStreamEvent::ActiveSpecUpdated => {
            options
                .mp
                .note("active spec changed - reloading environment");
        }
        UpdateStreamEvent::Finish(s) => {
            if !s.stream_updated {
                if let Some(stream_update_spinner) = stream_update_spinner.as_ref() {
                    stream_update_spinner.finish_and_clear();
                }
            }
        }
    })?;

    Ok(())
}
