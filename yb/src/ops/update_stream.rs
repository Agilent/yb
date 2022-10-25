use std::collections::HashSet;

use crate::config::Config;
use crate::core::tool_context::require_yb_env;
use crate::errors::YbResult;
use crate::stream_db::StreamKey;
use crate::yb_env::ActiveSpecStatus;

#[derive(Default)]
pub struct UpdateStreamResult {
    pub active_spec_updated: bool,
}

pub enum UpdateStreamEvent<'a> {
    Start,
    ActiveSpecUpdated,
    Finish(&'a UpdateStreamResult),
}

pub struct UpdateStreamOptions<'cfg> {
    pub(crate) config: &'cfg Config,
    stream_keys: HashSet<StreamKey>,
}

impl<'cfg> UpdateStreamOptions<'cfg> {
    pub fn new(config: &'cfg Config, stream_keys: HashSet<StreamKey>) -> Self {
        Self {
            config,
            stream_keys,
        }
    }
}

pub fn op_update_stream<F>(options: UpdateStreamOptions, mut c: F) -> YbResult<UpdateStreamResult>
where
    F: FnMut(UpdateStreamEvent),
{
    let mut result = UpdateStreamResult::default();

    let arena = toolshed::Arena::new();
    let mut yb_env = require_yb_env(options.config, &arena)?;

    let active_spec_stream = yb_env.active_spec_status().and_then(|status| match status {
        ActiveSpecStatus::StreamsBroken(..) => None,
        ActiveSpecStatus::Active(spec) => Some(spec.stream_key),
    });

    c(UpdateStreamEvent::Start);

    for stream_key in options.stream_keys {
        let is_active_stream = active_spec_stream
            .map(|key| key == stream_key)
            .unwrap_or_default();

        {
            let stream = yb_env.stream_db_mut().stream_mut(stream_key).unwrap();
            stream.pull()?;
        }

        if is_active_stream {
            if let ActiveSpecStatus::Active(active_spec) =
                yb_env.active_spec_status().cloned().unwrap()
            {
                let stream = yb_env.stream_db().stream(stream_key).unwrap();
                let reloaded_spec = stream.get_spec_by_name(active_spec.spec.name()).unwrap();
                if *reloaded_spec != active_spec.spec {
                    c(UpdateStreamEvent::ActiveSpecUpdated);
                    result.active_spec_updated = true;
                    yb_env.activate_spec(reloaded_spec.clone())?;
                }
            } else {
                unreachable!();
            }
        }
    }

    c(UpdateStreamEvent::Finish(&result));

    Ok(result)
}
