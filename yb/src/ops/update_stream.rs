use std::borrow::Borrow;

use crate::config::Config;
use crate::core::tool_context::require_yb_env;
use crate::errors::YbResult;
use crate::yb_env::ActiveSpecStatus;

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
    //let mut reloaded_active_spec = None;
    let mut result = UpdateStreamResult::default();

    let arena = toolshed::Arena::new();
    let mut yb_env = require_yb_env(options.config, &arena)?;

    match yb_env.active_spec_status() {
        None => {},
        Some(status) => {
            match status {
                ActiveSpecStatus::StreamBroken => {

                },
                ActiveSpecStatus::Active(spec) => {
                    let stream = yb_env.active_stream_mut();

                }
            }
        }
    }

    // if yb_env.active_spec_status().has_active_spec() {
    //     let mut a = false;
    //     if let Some(stream) = yb_env.active_stream_mut() {
    //         c(UpdateStreamEvent::Start);
    //
    //         stream.pull()?;
    //         a = true;
    //         // if *stream != *reloaded_stream.borrow() {
    //         //     result.stream_updated = true;
    //         //     c(UpdateStreamEvent::ActiveStreamUpdated);
    //         // }
    //     } else {
    //         eyre::bail!("active spec refers to nonexistent stream?");
    //     }
    //
    //     if a {
    //
    //         let active_spec = yb_env.active_spec_status().unwrap();
    //         if let Some(reloaded_spec) = yb_env.stream_db().find_spec_by_name(active_spec.spec.name())? {
    //             if active_spec.spec != *reloaded_spec {
    //                 result.spec_updated = true;
    //                 reloaded_active_spec = Some(reloaded_spec.clone());
    //             }
    //         } else {
    //             eyre::bail!(
    //                 "spec {} no longer exists in reloaded stream",
    //                 active_spec.spec.name()
    //             );
    //         }
    //
    //         if let Some(reloaded_active_spec) = reloaded_active_spec {
    //             c(UpdateStreamEvent::ActiveSpecUpdated);
    //             yb_env.activate_spec(reloaded_active_spec)?;
    //         }
    //     }
    // }

    c(UpdateStreamEvent::Finish(&result));

    Ok(result)
}
