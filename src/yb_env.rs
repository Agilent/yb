use core::fmt::{self, Debug, Formatter};
use eyre::Context;
use std::collections::hash_map::Iter;
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use walkdir::WalkDir;

use crate::core::tool_context::YoctoEnvironment;
use crate::errors::YbResult;
use crate::spec::{ActiveSpec, Spec};
use crate::stream::Stream;
use crate::util::paths::{find_dir_recurse_upwards, is_hidden};
use crate::yb_conf::YbConf;

const YB_ENV_DIRECTORY: &str = ".yb";
const STREAMS_SUBDIR: &str = "streams";
const YB_CONF_FILE: &str = "yb.yaml";
const ACTIVE_SPEC_FILE: &str = "active_spec.yaml";

pub enum ConfigActiveSpecStatus {
    ActiveSpec { name: String },
    NoActiveSpec,
    NoYbEnv,
}

impl ConfigActiveSpecStatus {
    pub fn has_active_spec(&self) -> bool {
        matches!(&self, ConfigActiveSpecStatus::ActiveSpec { .. })
    }
}

pub struct YbEnv<'arena> {
    /// Absolute path to the .yb directory
    dir: PathBuf,
    config: YbConf,
    active_spec: Option<ActiveSpec>,
    streams_by_name: HashMap<String, Rc<Stream>>,
    // TODO: remove if not going to use it
    _placeholder: PhantomData<&'arena str>,
}

impl<'arena> Debug for YbEnv<'arena> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("YbEnv")
            .field("dir", &self.dir)
            .field("config", &self.config)
            .field("active_spec", &self.active_spec)
            .field("streams_by_name", &self.streams_by_name)
            .finish_non_exhaustive()
    }
}

impl<'arena> YbEnv<'arena> {
    fn new(
        dir: PathBuf,
        config: YbConf,
        active_spec: Option<ActiveSpec>,
        streams: HashMap<String, Rc<Stream>>,
        _arena: &'arena toolshed::Arena,
    ) -> Self {
        Self {
            dir,
            config,
            active_spec,
            streams_by_name: streams,
            _placeholder: PhantomData,
        }
    }

    pub fn streams_by_name(&self) -> Iter<'_, String, Rc<Stream>> {
        self.streams_by_name.iter()
    }

    pub fn activate_spec(&mut self, spec: Spec) -> YbResult<()> {
        let active_spec = ActiveSpec::from(spec);
        let dest = self.dir.join(ACTIVE_SPEC_FILE);
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&dest)?;
        serde_yaml::to_writer(&f, &active_spec)?;

        self.active_spec = Some(active_spec);
        Ok(())
    }

    pub fn active_stream(&self) -> Option<Rc<Stream>> {
        // TODO: should be sanity check that this never be None
        self.active_spec
            .as_ref()
            .and_then(|s| s.weak_stream.upgrade())
    }

    pub fn find_spec<S: AsRef<str>>(&self, name: S) -> YbResult<Option<&Spec>> {
        let mut ret = None;
        for stream in self.streams_by_name.values() {
            let s = stream.get_spec_by_name(&name);
            if s.is_some() {
                if ret.is_some() {
                    eyre::bail!("spec '{}' found in multiple streams", name.as_ref());
                }
                ret = s;
            }
        }

        Ok(ret)
    }

    pub fn root_dir(&self) -> &PathBuf {
        &self.dir
    }

    pub fn active_spec(&self) -> Option<&ActiveSpec> {
        self.active_spec.as_ref()
    }

    pub fn active_spec_status(&self) -> ConfigActiveSpecStatus {
        // TODO: pull out of here, can't handle NoYbEnv
        match self.active_spec() {
            None => ConfigActiveSpecStatus::NoActiveSpec,
            Some(active_spec) => ConfigActiveSpecStatus::ActiveSpec {
                name: active_spec.name(),
            },
        }
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
        arena: &'arena toolshed::Arena,
    ) -> YbResult<YbEnv<'arena>> {
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

        Ok(YbEnv::new(yb_dir, conf, None, HashMap::new(), arena))
    }
}

/// Search upwards from `start_point` for a .yb directory and load the environment if found.
pub fn try_discover_yb_env<S: AsRef<Path>>(
    start_point: S,
    arena: &toolshed::Arena,
) -> YbResult<Option<YbEnv>> {
    // Locate the hidden .yb directory
    find_dir_recurse_upwards(start_point, YB_ENV_DIRECTORY)?.map(|yb_dir| -> YbResult<_> {
        tracing::info!("found .yb directory at {:?}", yb_dir);
        let conf_file = yb_dir.join(YB_CONF_FILE);
        // TODO handle missing conf file?
        assert!(conf_file.is_file());

        let mut config_file_data = Vec::new();
        File::open(&conf_file)
            .with_context(|| {
                format!("failed to open conf file {}", conf_file.display())
            })?
            .read_to_end(&mut config_file_data)?;

        let conf: YbConf = serde_yaml::from_slice(config_file_data.as_slice()).unwrap();

        let mut active_spec;
        let active_spec_file_path = yb_dir.join(ACTIVE_SPEC_FILE);
        if active_spec_file_path.is_file() {
            let active_spec_file = File::open(&active_spec_file_path)?;
            active_spec = Some(
                serde_yaml::from_reader::<_, ActiveSpec>(active_spec_file)
                    .with_context(|| {
                        format!(
                            "failed to parse active spec file {}",
                            &active_spec_file_path.display()
                        )
                    })?,
            );
        } else {
            active_spec = None;
        }

        let mut streams_by_name = HashMap::new();

        let streams_dir = yb_dir.join(STREAMS_SUBDIR);
        if streams_dir.is_dir() {
            // Iterate over each stream (which are subdirectories)
            for d in WalkDir::new(streams_dir)
                .max_depth(1)
                .min_depth(1)
                .into_iter()
                .filter_entry(|e| !is_hidden(e))
                .filter(|e| e.as_ref().unwrap().file_type().is_dir())
            {
                let stream_path = d?.into_path();
                let stream = Stream::load(stream_path.clone())?;
                streams_by_name.insert(
                    stream_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                    stream,
                );
            }

            if let Some(active_spec) = &mut active_spec {
                if let Some(stream) = streams_by_name.get(&*active_spec.from_stream) {
                    if stream.get_spec_by_name(active_spec.name()).is_none() {
                        eyre::bail!("active spec '{}' claims to be a member of stream '{}', but it was not found there", active_spec.name(), active_spec.from_stream);
                    }

                    active_spec.weak_stream = Rc::downgrade(stream);
                } else {
                    eyre::bail!(
                        "active spec '{}' refers to non-existent stream '{}'",
                        active_spec.name(),
                        active_spec.from_stream
                    );
                }
            }

            // TODO ensure the spec is actually contained in the stream it says it is
        } else if let Some(active_spec) = &active_spec {
            eyre::bail!(
                "spec '{}' is active, but there are no streams?",
                active_spec.name()
            );
        }

        return Ok(YbEnv::new(
            yb_dir,
            conf,
            active_spec,
            streams_by_name,
            arena,
        ));
    }).transpose()
}
