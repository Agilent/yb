use color_eyre::{Help};
use std::env;

use std::path::{Path, PathBuf};

use git2::Repository;


use crate::config::Config;
use crate::errors::YbResult;
use crate::util::paths::{list_subdirectories_sorted, run_which};
use crate::yb_env::{try_discover_yb_env, YbEnv};

#[derive(Debug)]
pub enum ToolContext<'arena> {
    Yb(YbEnv<'arena>),
    YoctoEnv(YoctoEnvironment),
}

impl<'arena> ToolContext<'arena> {
    /// Returns a `Vec` of `Repository` objects representing each top-level git repo found in
    /// the sources directory.
    pub fn sources_repos(&self) -> YbResult<Vec<Repository>> {
        list_subdirectories_sorted(&self.sources_dir()).map(|subdirs| {
            subdirs
                .iter()
                .filter_map(|d| {
                    // TODO: this throws out every error - maybe need to be more careful
                    Repository::discover(d).ok()
                })
                .collect()
        })
    }

    pub fn sources_dir(&self) -> PathBuf {
        match self {
            ToolContext::Yb(yb_env) => yb_env.sources_dir(),
            ToolContext::YoctoEnv(yocto_env) => yocto_env.sources_dir.clone(),
        }
    }

    pub fn build_dir(&self) -> PathBuf {
        match self {
            ToolContext::Yb(yb_env) => yb_env.build_dir(),
            ToolContext::YoctoEnv(yocto_env) => yocto_env.build_dir.clone(),
        }
    }
}

#[derive(Debug)]
pub struct YoctoEnvironment {
    pub(crate) build_dir: PathBuf,
    pub(crate) poky_layer: Option<PathBuf>,
    pub(crate) sources_dir: PathBuf,
}

pub fn determine_tool_context<'arena>(
    config: &Config,
    arena: &'arena toolshed::Arena,
) -> YbResult<Option<ToolContext<'arena>>> {
    if run_which("petalinux-build")?.is_some() {
        eyre::bail!("PetaLinux is not supported, but an active PetaLinux environment was detected");
    }

    // Figure out what kind of context we are executing under
    if let Some(yb_env) = try_discover_yb_env(config.cwd(), arena)? {
        // A .yb directory was found
        return Ok(Some(ToolContext::Yb(yb_env)));
    } else {
        // Check for activated Yocto environment
        let bbpath = env::var("BBPATH").ok().map(PathBuf::from);
        let poky_layer_maybe = run_which("oe-buildenv-internal")?
            .and_then(|s| s.parent().map(|p| p.to_path_buf()))
            .and_then(|s| s.parent().map(|p| p.to_path_buf()));

        // Assume all other repos are siblings of the poky layer
        let sources_dir = poky_layer_maybe
            .as_ref()
            .map(|l| l.parent().map(|p| p.to_path_buf()).unwrap());

        match (&bbpath, &poky_layer_maybe, &sources_dir) {
            (Some(build_dir), Some(poky_layer), Some(sources_dir)) => {
                // Check for bare Poky environments
                if let Some(build_dir_parent) = &build_dir.parent().map(Path::to_path_buf) {
                    if poky_layer == build_dir_parent {
                        eyre::bail!("Bare poky environments are not supported");
                    }
                }

                return Ok(Some(ToolContext::YoctoEnv(YoctoEnvironment {
                    sources_dir: sources_dir.clone(),
                    build_dir: build_dir.clone(),
                    poky_layer: Some(poky_layer.clone()),
                })));
            }
            (None, None, None) => {}
            _ => {
                eyre::bail!(
                    "Found partially activated Yocto environment? {:?} {:?} {:?}",
                    &bbpath,
                    &poky_layer_maybe,
                    &sources_dir
                );
            }
        }
    }

    Ok(None)
}

pub fn require_tool_context<'arena>(
    config: &Config,
    arena: &'arena toolshed::Arena,
) -> YbResult<ToolContext<'arena>> {
    determine_tool_context(config, arena).and_then(|c| {
        c.ok_or_else(|| {
            tracing::error!("expected a yb or Yocto environment");
            eyre::eyre!("expected a yb or Yocto environment").suggestion("use yb init").suppress_backtrace(true)
        })
    })
}

pub fn require_yb_env<'arena>(
    config: &Config,
    arena: &'arena toolshed::Arena,
) -> YbResult<YbEnv<'arena>> {
    determine_tool_context(config, arena).and_then(|c| match c {
        None => eyre::bail!("expected a yb environment; no environment was found"),
        Some(ToolContext::Yb(yb_env)) => Ok(yb_env),
        Some(ToolContext::YoctoEnv(_)) => {
            eyre::bail!("expected a yb environment; a Yocto environment was found")
        }
    })
}

pub fn maybe_yb_env<'arena>(
    config: &Config,
    arena: &'arena toolshed::Arena,
) -> YbResult<Option<YbEnv<'arena>>> {
    let ret = determine_tool_context(config, arena).map(|c| {
        if let Some(ToolContext::Yb(yb_env)) = c {
            Some(yb_env)
        } else {
            None
        }
    });
    ret
}
