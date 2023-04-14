use async_trait::async_trait;
use color_eyre::Help;
use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::core::tool_context::{determine_tool_context, require_yb_env, ToolContext};
use crate::errors::YbResult;
use crate::ops::add_stream::{op_add_stream, AddStreamOptions};
use crate::yb_env::YbEnv;
use crate::Config;

/// Create a 'yb' environment within an activated Yocto environment
///
/// When run in the context of an activated Yocto environment (i.e. the command `bitbake` is
/// available in your terminal), the .yb control directory is created above the top-level repos
/// directory (typically 'sources'). For example if your layers live in yocto/sources/ then the
/// control directory is created at yocto/.yb
#[derive(Debug, clap::Parser)]
#[clap(verbatim_doc_comment)]
pub struct UpgradeCommand {
    /// You can use the '--default-stream' flag to specify a default spec stream to be added.
    ///
    /// URI pointing to a default spec stream to add
    #[clap(name = "default-stream", short = 's', long)]
    default_stream: Option<String>,

    #[clap(name = "default-spec", short = 'p', long, requires = "default-stream")]
    default_spec: Option<String>,
}

#[async_trait]
impl SubcommandRunner for UpgradeCommand {
    async fn run(&self, config: &mut Config, _mp: &MultiProgress) -> YbResult<()> {
        let context = determine_tool_context(config)?;

        match context {
            Some(ToolContext::Yb(yb_env)) => {
                return Err(eyre::eyre!(
                    "a .yb environment already exists at {:?}",
                    yb_env.root_dir()
                ));
            }
            Some(ToolContext::YoctoEnv(context2)) => {
                // An activated Yocto environment
                let target = context2.sources_dir.parent().unwrap().to_owned();

                // Sanity check: make sure cwd is under `target`
                if !config.cwd.ancestors().any(|ancestor| ancestor == target) {
                    return Err(eyre::eyre!(
                    "current working directory must be within the activated Yocto environment to proceed",
                )
                        .suggestion(format!("`cd` to the Yocto environment ({}) and then try again", target.display()))
                        .suggestion("or, activate a different Yocto environment")
                        .suppress_backtrace(true)
                    );
                }

                let new_context = ToolContext::Yb(YbEnv::initialize(target, &context2)?);
                match &new_context {
                    ToolContext::Yb(yb_env) => println!("initialized yb env at {yb_env:?}"),
                    _ => panic!(""),
                };
            }
            None => {
                return Err(eyre::eyre!(
                    "an activated Yocto environment was not found",
                ).suggestion("use `yb init` if you want to create a fresh yb and Yocto env")
                    .suggestion("or, if you meant to use `yb upgrade`, make sure your Yocto env is activated")
                    .suppress_backtrace(true)
                );
            }
        };

        if let Some(default_stream_uri) = &self.default_stream {
            let mut add_stream_opts = AddStreamOptions::new(config);
            add_stream_opts.uri(default_stream_uri.clone());
            op_add_stream(add_stream_opts)?;

            if let Some(default_spec_name) = &self.default_spec {
                // TODO deduplicate code
                let mut yb_env = require_yb_env(config)?;

                let spec = yb_env.find_spec(default_spec_name)?.cloned();
                if let Some(spec) = spec {
                    // TODO don't clone
                    yb_env.activate_spec(spec)?;
                    println!("Activated spec '{}'", &default_spec_name);
                } else {
                    eyre::bail!("spec with name '{}' not found", &default_spec_name);
                }
            }
        }

        Ok(())
    }
}
