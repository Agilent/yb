use std::collections::hash_map::Iter;
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
use crate::util::git::{
    do_merge, get_current_local_branch_name, get_remote_name_for_current_branch,
    ssh_agent_remote_callbacks,
};
use crate::util::paths::{is_hidden, is_yaml_file};

// TODO: don't make pub, move logic here
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
    specs: HashMap<String, Spec>,
    repo: Mutex<Repository>,
    config: StreamConfig,
}

impl Debug for Stream {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stream")
            .field("path", &self.path)
            .field("name", &self.name)
            .field("specs", &self.specs)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Stream {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.name == other.name
            && self.specs == other.specs
            && self.config == other.config
    }
}

impl Eq for Stream {}

impl Stream {
    pub fn load(path: PathBuf) -> YbResult<Arc<Self>> {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let mut specs = HashMap::new();

        let stream_contents = path.join(STREAM_CONTENT_ROOT_SUBDIR);
        for spec_yaml in WalkDir::new(&stream_contents)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .filter(|e| is_yaml_file(e.as_ref().unwrap()))
        {
            let spec_path = spec_yaml?.into_path();
            let spec = Spec::load(name.clone(), &spec_path)?;
            specs.insert(spec.name(), spec);
        }

        let f = File::open(path.join(STREAM_CONFIG_FILE))?;
        let config = serde_yaml::from_reader::<_, StreamConfig>(&f)?;

        let repo = Repository::discover(&stream_contents)?;

        Ok(Arc::new_cyclic(|self_weak| {
            let specs = specs
                .drain()
                .map(|(name, mut spec)| {
                    spec.weak_stream = self_weak.clone();
                    (name, spec)
                })
                .collect();

            Stream {
                path,
                name,
                specs,
                repo: Mutex::new(repo),
                config,
            }
        }))
    }

    pub fn reload(&self) -> YbResult<Arc<Self>> {
        let repo = &self.repo.lock().unwrap();

        let upstream_name = get_remote_name_for_current_branch(repo)?.unwrap();
        let current_branch_name = get_current_local_branch_name(repo)?;

        let mut remote = repo.find_remote(&upstream_name)?;
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(ssh_agent_remote_callbacks());
        remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

        do_merge(repo, &current_branch_name, fetch_commit)?;

        Self::load(self.path.clone())
    }

    pub fn specs_by_name(&self) -> Iter<'_, String, Spec> {
        self.specs.iter()
    }

    pub fn get_spec_by_name<S: AsRef<str>>(&self, name: S) -> Option<&Spec> {
        self.specs.get(name.as_ref())
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}
