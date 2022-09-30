use async_trait::async_trait;
use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::errors::YbResult;
use crate::ops::add_stream::{op_add_stream, AddStreamOptions};
use crate::Config;

#[derive(Debug, clap::Parser)]
pub struct StreamAddCommand {
    #[clap()]
    uri: String,

    #[clap(long, short)]
    name: Option<String>,
}

#[async_trait]
impl SubcommandRunner for StreamAddCommand {
    async fn run(&self, config: &mut Config, _mp: &MultiProgress) -> YbResult<()> {
        let mut add_stream_opts = AddStreamOptions::new(config);
        add_stream_opts.name(self.name.clone());
        add_stream_opts.uri(self.uri.clone());
        op_add_stream(add_stream_opts)
    }
}
