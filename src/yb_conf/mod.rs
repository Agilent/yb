use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::tool_context::YoctoEnvironment;
use crate::errors::YbResult;
use crate::util::paths::try_diff_paths;

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


#[cfg(test)]
mod test {
    use crate::yb_conf::{YB_CONF_FORMAT_VERSION, YbConf};

    #[test]
    fn fake_version_1_handling() {
        // This is actually version 2, but I never bumped the format version :/
        let conf = r#"---
format_version: 1
build_dir_relative: "../build"
sources_dir_relative: "../sources"
poky_dir_relative: "../sources/poky"
"#;

        let yb_conf: YbConf = serde_yaml::from_str(conf).unwrap();
        assert_eq!(yb_conf.format_version, 1);
    }

    #[test]
    fn version_1_handling() {
        let conf = r#"---
format_version: 1
build_dir_relative: "../build"
repos_dir_relative: "../sources"
poky_dir_relative: "../sources/poky"
"#;

        let yb_conf: YbConf = serde_yaml::from_str(conf).unwrap();
        assert_eq!(yb_conf.format_version, 1);
    }

    #[test]
    fn format_version_up_to_date() {
        assert_eq!(YB_CONF_FORMAT_VERSION, 2, "need to update migration code!");
    }
}

