use core::fmt::{self, Debug, Formatter};
use eyre::Context;
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::tool_context::YoctoEnvironment;
use crate::errors::YbResult;
use crate::spec::{ActiveSpec, Spec};
use crate::stream::Stream;
use crate::stream_db::{StreamDb, StreamKey};
use crate::util::paths::find_dir_recurse_upwards;
use crate::yb_conf::YbConf;

const YB_ENV_DIRECTORY: &str = ".yb";
const STREAMS_SUBDIR: &str = "streams";
const YB_CONF_FILE: &str = "yb.yaml";
const ACTIVE_SPEC_FILE: &str = "active_spec.yaml";

#[derive(Debug, Clone)]
pub enum ActiveSpecStatus {
    Active(ActiveSpec),
    StreamsBroken(HashMap<StreamKey, Arc<eyre::Report>>),
}

pub struct YbEnv {
    /// Absolute path to the .yb directory
    dir: PathBuf,
    config: YbConf,
    active_spec_status: Option<ActiveSpecStatus>,
    streams: StreamDb,
}

impl Debug for YbEnv {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("YbEnv")
            .field("dir", &self.dir)
            .field("config", &self.config)
            .field("active_spec_status", &self.active_spec_status)
            .field("streams", &self.streams)
            .finish_non_exhaustive()
    }
}

impl YbEnv {
    fn new(
        dir: PathBuf,
        config: YbConf,
        active_spec: Option<ActiveSpecStatus>,
        streams: StreamDb,
    ) -> Self {
        Self {
            dir,
            config,
            active_spec_status: active_spec,
            streams,
        }
    }

    pub fn stream_db(&self) -> &StreamDb {
        &self.streams
    }

    pub fn stream_db_mut(&mut self) -> &mut StreamDb {
        &mut self.streams
    }

    pub fn activate_spec(&mut self, spec: Spec) -> YbResult<()> {
        let active_spec = self.streams.make_active_spec(spec)?;

        let dest = self.dir.join(ACTIVE_SPEC_FILE);
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dest)?;
        serde_yaml::to_writer(&f, &active_spec)?;

        self.active_spec_status = Some(ActiveSpecStatus::Active(active_spec));
        Ok(())
    }

    pub fn active_stream_mut(&mut self) -> Option<&mut Stream> {
        let key = self.active_spec_status.as_ref().and_then(|a| match a {
            ActiveSpecStatus::Active(spec) => Some(spec.stream_key),
            _ => None,
        });

        key.and_then(move |k| self.streams.stream_mut(k))
    }

    pub fn find_spec<S: AsRef<str>>(&self, name: S) -> YbResult<Option<&Spec>> {
        self.streams.find_spec_by_name(name)
    }

    pub fn root_dir(&self) -> &PathBuf {
        &self.dir
    }

    pub fn active_spec_status(&self) -> Option<&ActiveSpecStatus> {
        self.active_spec_status.as_ref()
    }

    pub fn build_dir(&self) -> PathBuf {
        self.dir.join(self.config.build_dir_relative())
    }

    pub fn sources_dir(&self) -> PathBuf {
        self.dir.join(self.config.sources_dir_relative())
    }

    pub fn poky_dir(&self) -> Option<PathBuf> {
        self.config.poky_dir_relative().map(|p| self.dir.join(p))
    }

    pub fn yb_dir(&self) -> &PathBuf {
        &self.dir
    }

    pub fn streams_dir(&self) -> PathBuf {
        self.yb_dir().join(STREAMS_SUBDIR)
    }

    pub fn initialize<S: Into<PathBuf>>(
        location: S,
        yocto_env: &YoctoEnvironment,
    ) -> YbResult<YbEnv> {
        // TODO: create in temp directory then move over?
        let yb_dir = location.into().join(YB_ENV_DIRECTORY);
        println!("creating at {:?}", &yb_dir);
        fs::create_dir(&yb_dir)?;

        // Create a default yb.yaml file and write it to disk
        let conf = YbConf::new_from_yocto_env(&yb_dir, yocto_env)?;
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .open(yb_dir.join(YB_CONF_FILE))?;
        serde_yaml::to_writer(f, &conf)?;

        // Create the streams dir
        let streams_dir = yb_dir.join(STREAMS_SUBDIR);
        fs::create_dir(streams_dir)?;

        Ok(YbEnv::new(yb_dir, conf, None, StreamDb::new()))
    }
}

/// Search upwards from `start_point` for a .yb directory and load the environment if found.
pub fn try_discover_yb_env<S: AsRef<Path>>(
    start_point: S,
) -> YbResult<Option<YbEnv>> {
    // Locate the hidden .yb directory
    find_dir_recurse_upwards(start_point, YB_ENV_DIRECTORY)?
        .map(|yb_dir| -> YbResult<_> {
            tracing::info!("found .yb directory at {:?}", yb_dir);
            let conf_file = yb_dir.join(YB_CONF_FILE);
            // TODO handle missing conf file?
            assert!(conf_file.is_file());

            let mut config_file_data = Vec::new();
            File::open(&conf_file)
                .with_context(|| format!("failed to open conf file {}", conf_file.display()))?
                .read_to_end(&mut config_file_data)?;

            let conf: YbConf = serde_yaml::from_slice(config_file_data.as_slice()).unwrap();

            let mut stream_db = StreamDb::new();

            let streams_dir = yb_dir.join(STREAMS_SUBDIR);
            if streams_dir.is_dir() {
                stream_db.load_all(streams_dir)?;
            }

            let active_spec;
            let broken_streams = stream_db.broken_streams();
            if !broken_streams.is_empty() {
                // TODO: active spec is not necessarily part of broken stream(s); could still try to load it
                active_spec = Some(ActiveSpecStatus::StreamsBroken(broken_streams));
            } else {
                let active_spec_file_path = yb_dir.join(ACTIVE_SPEC_FILE);
                if active_spec_file_path.is_file() {
                    active_spec = Some(ActiveSpecStatus::Active(
                        stream_db.load_active_spec(active_spec_file_path)?,
                    ));
                } else {
                    active_spec = None;
                }
            }

            Ok(YbEnv::new(yb_dir, conf, active_spec, stream_db))
        })
        .transpose()
}
