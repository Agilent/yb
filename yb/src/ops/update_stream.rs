use std::borrow::Borrow;

use crate::config::Config;
use crate::core::tool_context::require_yb_env;
use crate::errors::YbResult;

#[derive(Default)]
pub struct UpdateStreamResult {
    pub stream_updated: bool,
    pub spec_updated: bool,
}

pub enum UpdateStreamEvent<'a> {
    Start,
    ActiveStreamUpdated,
    ActiveSpecUpdated,
    Finish(&'a UpdateStreamResult),
}

pub struct UpdateStreamOptions<'cfg> {
    pub(crate) config: &'cfg Config,
}

impl<'cfg> UpdateStreamOptions<'cfg> {
    pub fn new(config: &'cfg Config) -> Self {
        Self { config }
    }
}

pub fn op_update_stream<F>(options: UpdateStreamOptions, mut c: F) -> YbResult<UpdateStreamResult>
where
    F: FnMut(UpdateStreamEvent),
{
    let mut reloaded_active_spec = None;
    let mut result = UpdateStreamResult::default();

    let arena = toolshed::Arena::new();
    let mut yb_env = require_yb_env(options.config, &arena)?;
    if yb_env.active_spec_status().has_active_spec() {
        if let Some(stream) = yb_env.active_stream() {
            c(UpdateStreamEvent::Start);

            let reloaded_stream = stream.reload()?;
            if *stream != *reloaded_stream.borrow() {
                result.stream_updated = true;
                c(UpdateStreamEvent::ActiveStreamUpdated);
            }

            let active_spec = yb_env.active_spec().unwrap();
            if let Some(reloaded_spec) = reloaded_stream.get_spec_by_name(active_spec.spec.name()) {
                if active_spec.spec != *reloaded_spec {
                    result.spec_updated = true;
                    reloaded_active_spec = Some(reloaded_spec.clone());
                }
            } else {
                eyre::bail!(
                    "spec {} no longer exists in reloaded stream",
                    active_spec.spec.name()
                );
            }

            if let Some(reloaded_active_spec) = reloaded_active_spec {
                c(UpdateStreamEvent::ActiveSpecUpdated);
                yb_env.activate_spec(reloaded_active_spec)?;
            }
        } else {
            eyre::bail!("active spec refers to nonexistent stream?");
        }
    }

    c(UpdateStreamEvent::Finish(&result));

    Ok(result)
}
