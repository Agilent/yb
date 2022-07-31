use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::tool_context::YoctoEnvironment;
use crate::errors::YbResult;
use crate::util::paths::try_diff_paths;

pub mod migrations;

pub const YB_CONF_FORMAT_VERSION: u32 = 2;

#[derive(Debug, Serialize, Deserialize)]
pub struct YbConf {
    format_version: u32,

    /// Location of the build directory relative to .yb directory
    build_dir_relative: PathBuf,

    /// Location of the top-level sources directory relative to the .yb directory
    #[serde(alias = "repos_dir_relative")]
    sources_dir_relative: PathBuf,

    /// Location of the poky layer relative to the .yb directory
    poky_dir_relative: Option<PathBuf>,
}

impl YbConf {
    pub fn new_from_yocto_env(yb_dir: &Path, yocto_env: &YoctoEnvironment) -> YbResult<Self> {
        // There may not be a poky directory (or any layers) yet
        let poky_dir_relative = yocto_env
            .poky_layer
            .as_ref()
            .map(|p| try_diff_paths(p, yb_dir))
            .map_or(Ok(None), |r| r.map(Some))?;

        Ok(YbConf {
            format_version: YB_CONF_FORMAT_VERSION,
            build_dir_relative: try_diff_paths(&yocto_env.build_dir, yb_dir)?,
            sources_dir_relative: try_diff_paths(&yocto_env.sources_dir, yb_dir)?,
            poky_dir_relative,
        })
    }

    pub fn build_dir_relative(&self) -> &PathBuf {
        &self.build_dir_relative
    }

    pub fn sources_dir_relative(&self) -> &PathBuf {
        &self.sources_dir_relative
    }

    pub fn poky_dir_relative(&self) -> Option<&PathBuf> {
        self.poky_dir_relative.as_ref()
    }
}

pub fn load_yb_conf_current_version_only<R>(f: R) -> YbResult<YbConf>
where
    R: io::Read,
{
    serde_yaml::from_reader::<_, YbConf>(f).or_else(|e| Err(e.into()))
}
