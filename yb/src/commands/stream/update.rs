use async_trait::async_trait;
use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::errors::YbResult;
use crate::ui_ops::update_stream::{ui_op_update_stream, UiUpdateStreamOptions};
use crate::Config;

#[derive(Debug, clap::Parser)]
pub struct StreamUpdateCommand {}

#[async_trait]
impl SubcommandRunner for StreamUpdateCommand {
    async fn run(&self, config: &mut Config, mp: &MultiProgress) -> YbResult<()> {
        let mut update_stream_opts = UiUpdateStreamOptions::new(config, mp);
        update_stream_opts.fail_if_no_yb_env(true);
        ui_op_update_stream(update_stream_opts)
    }
}
