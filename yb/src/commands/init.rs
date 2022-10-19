use async_trait::async_trait;
use color_eyre::Help;
use indicatif::MultiProgress;
use std::fs;

use crate::commands::SubcommandRunner;
use crate::core::tool_context::{
    determine_tool_context, require_yb_env, ToolContext, YoctoEnvironment,
};
use crate::errors::YbResult;
use crate::ops::add_stream::{op_add_stream, AddStreamOptions};
use crate::yb_env::YbEnv;
use crate::Config;

/// Initialize a 'yb' environment
///
/// When run in the context of an activated Yocto environment (e.g. you have sourced 'setupsdk'),
/// the .yb control directory is created above the top-level repos directory (typically 'sources').
/// For example if your layers live in yocto/sources then the control directory is created at yocto/.yb
///
/// If no Yocto environment is activated then a directory called 'yocto' is created, the .yb control directory
/// is initialized underneath it, and empty 'build' and 'sources' directories are created:
///
///     yocto/
///     ├── build
///     ├── sources
///     └── .yb
///
#[derive(Debug, clap::Parser)]
#[clap(verbatim_doc_comment)]
pub struct InitCommand {
    /// You can use the '--default-stream' flag to specify a default spec stream to be added.
    ///
    /// URI pointing to a default spec stream to add
    #[clap(name = "default-stream", short = 's', long)]
    default_stream: Option<String>,

    #[clap(name = "default-spec", short = 'p', long, requires = "default-stream")]
    default_spec: Option<String>,
}

#[async_trait]
impl SubcommandRunner for InitCommand {
    async fn run(&self, config: &mut Config, _mp: &MultiProgress) -> YbResult<()> {
        let arena = toolshed::Arena::new();
        let context = determine_tool_context(config, &arena)?;

        let new_yocto_dir;
        match context {
            Some(ToolContext::Yb(yb_env)) => {
                return Err(eyre::eyre!(
                    "a .yb environment already exists at {:?}",
                    yb_env.root_dir()
                ));
            }
            Some(ToolContext::YoctoEnv(context2)) => {
                return Err(eyre::eyre!(
                    "cannot init yb env within an activated Yocto environment",
                ).suggestion("use `yb upgrade` if you want to create a yb env within your activated Yocto env")
                    .suggestion("or, if you meant to use `yb init`, launch a new shell")
                    .note(format!("Yocto environment in question is rooted at: {}", context2.sources_dir.parent().unwrap().to_owned().display()))
                    .warning("since 0.0.12, the behavior of yb init has changed. See https://github.com/Agilent/yb/issues/3")
                    .suppress_backtrace(true)
                );
            }
            None => {
                // No environment, create a skeleton one
                let yocto_dir = config.cwd().join("yocto");
                new_yocto_dir = yocto_dir.clone();
                fs::create_dir(&yocto_dir)?;

                let sources_dir = yocto_dir.join("sources");
                let build_dir = yocto_dir.join("build");
                fs::create_dir(&sources_dir)?;
                fs::create_dir(&build_dir)?;

                let new_yocto_env = YoctoEnvironment {
                    build_dir,
                    sources_dir,
                    poky_layer: None,
                };

                let arena = toolshed::Arena::new();
                let yb_env = YbEnv::initialize(&yocto_dir, &new_yocto_env, &arena)?;
                println!(
                    "created skeleton Yocto environment at {:?}, yb env at {:?}",
                    &yocto_dir, yb_env
                );
            }
        };

        if let Some(default_stream_uri) = &self.default_stream {
            let config = config.clone_with_cwd(new_yocto_dir);

            let mut add_stream_opts = AddStreamOptions::new(&config);
            add_stream_opts.uri(default_stream_uri.clone());
            op_add_stream(add_stream_opts)?;

            if let Some(default_spec_name) = &self.default_spec {
                // TODO deduplicate code
                let arena = toolshed::Arena::new();
                let mut yb_env = require_yb_env(&config, &arena)?;

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
