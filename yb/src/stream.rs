use std::collections::HashMap;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::path::PathBuf;

use std::sync::{Arc, Mutex};

use git2::{FetchOptions, Repository};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::errors::YbResult;
use crate::spec::Spec;
use crate::stream_db::StreamKey;
use crate::util::git::{
    do_merge, get_current_local_branch_name, get_remote_name_for_current_branch,
    ssh_agent_remote_callbacks,
};
use crate::util::paths::{is_hidden, is_yaml_file};

// TODO: don't make pub, move logic here?
const STREAM_CONFIG_FILE_VERSION: u32 = 1;
pub const STREAM_CONTENT_ROOT_SUBDIR: &str = "contents";
pub const STREAM_CONFIG_FILE: &str = "stream.yaml";

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum StreamKind {
    Git,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct StreamConfig {
    kind: StreamKind,
    format_version: u32,
}

impl StreamConfig {
    pub fn new(kind: StreamKind) -> Self {
        StreamConfig {
            kind,
            format_version: STREAM_CONFIG_FILE_VERSION,
        }
    }
}

pub struct Stream {
    path: PathBuf,
    name: String,
    repo: Mutex<Repository>,
    config: StreamConfig,
    specs: StreamSpecs,
    key: StreamKey,
}

impl Debug for Stream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stream")
            .field("path", &self.path)
            .field("name", &self.name)
            .field("specs", &self.specs)
            .field("key", &self.key)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
//
// impl PartialEq for Stream {
//     fn eq(&self, other: &Self) -> bool {
//         self.path == other.path
//             && self.name == other.name
//             && self.specs == other.specs
//             && self.config == other.config
//     }
// }
//
// impl Eq for Stream {}

impl Stream {
    pub fn load(path: PathBuf, name: String, stream_key: StreamKey) -> YbResult<Self> {
        let f = File::open(path.join(STREAM_CONFIG_FILE))?;
        let config: StreamConfig = serde_yaml::from_reader(&f)?;

        let stream_contents_dir = path.join(STREAM_CONTENT_ROOT_SUBDIR);
        let repo = Repository::discover(&stream_contents_dir)?;

        Ok(
            Stream {
                path,
                name,
                specs: Self::load_specs(stream_contents_dir, stream_key)?,
                repo: Mutex::new(repo),
                config,
                key: stream_key,
        })
    }

    fn load_specs(stream_contents_dir: PathBuf, stream_key: StreamKey) -> YbResult<StreamSpecs> {
        let mut specs = HashMap::new();

        // Iterate over each spec yaml
        for spec_yaml in WalkDir::new(&stream_contents_dir)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .filter(|e| is_yaml_file(e.as_ref().unwrap()))
        {
            let spec_path = spec_yaml?.into_path();
            if let Ok(spec) = Spec::load(&spec_path, stream_key) {
                specs.insert(spec.name(), spec);
            } else {
                // Error encountered while loading spec
                return Ok(StreamSpecs::Broken);
            }
        }

        Ok(StreamSpecs::Loaded(specs))
    }

    pub fn fetch(&self) -> YbResult<()> {
        let repo = self.repo.lock().unwrap();

        let upstream_name = get_remote_name_for_current_branch(&repo)?.unwrap();

        let mut remote = repo.find_remote(&upstream_name)?;
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(ssh_agent_remote_callbacks());
        remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
        Ok(())
    }

    pub fn pull(&mut self) -> YbResult<()> {
        self.fetch()?;

        let repo = self.repo.lock().unwrap();
        let current_branch_name = get_current_local_branch_name(&repo)?;

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

        do_merge(&repo, &current_branch_name, fetch_commit)?;

        let stream_contents_dir = self.path.join(STREAM_CONTENT_ROOT_SUBDIR);
        self.specs = Self::load_specs(stream_contents_dir, self.key)?;

        Ok(())
    }

    pub fn get_spec_by_name<S: AsRef<str>>(&self, name: S) -> Option<&Spec> {
        match &self.specs {
            StreamSpecs::Loaded(specs) => specs.get(name.as_ref()),
            StreamSpecs::Broken => None,
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn key(&self) -> StreamKey {
        self.key
    }

    pub fn is_broken(&self) -> bool {
        matches!(self.specs, StreamSpecs::Broken)
    }
}


#[derive(Debug)]
pub enum StreamSpecs {
    Loaded(HashMap<String, Spec>),
    Broken,
}
