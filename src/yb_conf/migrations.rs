use eyre::Context;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::YbResult;
use crate::yb_conf::YbConf;

// TODO: get rid of migration nonsense.

#[derive(Debug, Serialize, Deserialize)]
pub struct YbConfVersion1 {
    format_version: u32,

    /// Location of the build directory relative to .yb directory
    build_dir_relative: PathBuf,

    /// Location of the top-level repos directory relative to the .yb directory
    repos_dir_relative: PathBuf,

    /// Location of the poky layer relative to the .yb directory
    poky_dir_relative: Option<PathBuf>,
}

type YbConfVersion2 = YbConf;

impl From<YbConfVersion1> for YbConfVersion2 {
    fn from(old: YbConfVersion1) -> Self {
        Self {
            format_version: 2,
            build_dir_relative: old.build_dir_relative,
            sources_dir_relative: old.repos_dir_relative,
            poky_dir_relative: old.poky_dir_relative,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct YbConfFormatVersionOnly {
    pub format_version: u32,
}

pub enum PossiblyMigratedYbConf {
    Migrated(YbConf),
    NotMigrated(YbConf),
}

pub fn load_yb_conf_with_migrations<R>(mut reader: R) -> YbResult<PossiblyMigratedYbConf>
where
    R: io::Read,
{
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    let format_version = serde_yaml::from_slice::<YbConfFormatVersionOnly>(buffer.as_slice())
        .context("couldn't read the format version - you should delete the .yb directory and re-initialize the environment with 'yb init'")?.format_version;

    // TODO: this code is relatively fragile and should be replaced with macros
    match &format_version {
        1 => {
            // A format version of "1" could very well actually be version 2, because yb 0.0.5
            // made a change to the format but didn't bump the version format
            if let Ok(as_version_one) = serde_yaml::from_slice::<YbConfVersion1>(buffer.as_slice())
            {
                return Ok(PossiblyMigratedYbConf::Migrated(as_version_one.into()));
            }

            if let Ok(mut as_version_two) =
                serde_yaml::from_slice::<YbConfVersion2>(buffer.as_slice())
            {
                // Mark it as having been migrated to trigger reserialization of the config file
                // with the correct format version.
                as_version_two.format_version = 2;
                return Ok(PossiblyMigratedYbConf::Migrated(as_version_two));
            }
        }
        2 => {
            if let Ok(as_version_two) = serde_yaml::from_slice::<YbConfVersion2>(buffer.as_slice())
            {
                return Ok(PossiblyMigratedYbConf::NotMigrated(as_version_two));
            }
        }
        _ => eyre::bail!("unknown format version {}", &format_version),
    }

    eyre::bail!("deserialization failed for version {}", &format_version);
}

#[cfg(test)]
mod test {
    use std::assert_matches::assert_matches;

    use crate::yb_conf::YB_CONF_FORMAT_VERSION;

    use super::*;

    #[test]
    fn fake_version_1_handling() {
        // This is actually version 2, but I never bumped the format version :/
        let conf = r#"---
format_version: 1
build_dir_relative: "../build"
sources_dir_relative: "../sources"
poky_dir_relative: "../sources/poky"
"#;

        let as_v1 = serde_yaml::from_str::<YbConfVersion1>(conf);
        assert_matches!(
            as_v1,
            Err(_),
            "should have failed to deserialize fake v1 conf as v1"
        );

        let as_v2 = serde_yaml::from_str::<YbConfVersion2>(conf);
        match &as_v2 {
            Ok(conf) => {
                assert_eq!(conf.format_version, 1);
                assert_eq!(conf.build_dir_relative.to_str(), Some("../build"));
                assert_eq!(conf.sources_dir_relative.to_str(), Some("../sources"));
                assert_eq!(
                    conf.poky_dir_relative.as_ref().map(|p| p.to_str()),
                    Some(Some("../sources/poky"))
                );
            }
            Err(e) => {
                panic!(
                    "should have succeeded to deserialize fake v1 conf as v2, got error: {:?}\n",
                    e
                );
            }
        }

        // test migration 1 => 2
        match load_yb_conf_with_migrations(conf.as_bytes()) {
            Ok(PossiblyMigratedYbConf::Migrated(conf)) => {
                assert_eq!(conf.format_version, 2);
                assert_eq!(conf.build_dir_relative.to_str(), Some("../build"));
                assert_eq!(conf.sources_dir_relative.to_str(), Some("../sources"));
                assert_eq!(
                    conf.poky_dir_relative.as_ref().map(|p| p.to_str()),
                    Some(Some("../sources/poky"))
                );
            }
            Ok(PossiblyMigratedYbConf::NotMigrated(_)) => {
                panic!("should have migrated fake v1 conf to v2, but it loaded without migration")
            }
            Err(e) => panic!("failed to load fake v1 conf, error: {:?}\n", e),
        };
    }

    #[test]
    fn format_version_up_to_date() {
        assert_eq!(YB_CONF_FORMAT_VERSION, 2, "need to update migration code!");
    }
}
