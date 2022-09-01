use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Weak;

use crate::data_model::Layer;
use serde::{Deserialize, Deserializer, Serialize};

use crate::errors::YbResult;
use crate::stream::Stream;

const SPEC_FORMAT_VERSION: u32 = 1;

const fn default_format_version() -> u32 {
    SPEC_FORMAT_VERSION
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Spec {
    header: SpecHeader,
    pub(crate) repos: HashMap<String, SpecRepo>,

    #[serde(skip)]
    pub(crate) weak_stream: Weak<Stream>,
}

impl PartialEq for Spec {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header && self.repos == other.repos
    }
}

impl Eq for Spec {}

impl Spec {
    pub fn load(_stream_name: String, path: &Path) -> YbResult<Self> {
        let f = File::open(path)?;
        serde_yaml::from_reader::<_, Self>(f).map_err(|e| e.into())
    }

    pub fn name(&self) -> String {
        self.header.name.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SpecHeader {
    #[serde(alias = "version", default = "default_format_version")]
    format_version: u32,
    name: String,
}

// https://github.com/serde-rs/serde/issues/1098#issuecomment-760711617
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SpecRepo {
    pub(crate) url: String,
    pub(crate) refspec: String,
    #[serde(
        rename = "extra-remotes",
        default,
        deserialize_with = "deserialize_null_default"
    )]
    pub(crate) extra_remotes: HashMap<String, SpecRemote>,
    // each entry is a layer name
    pub(crate) layers: Option<HashMap<String, ()>>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SpecRepoLayer {
    Root,
    Named(String),
}

impl SpecRepo {
    pub fn all_remotes(&self) {}

    pub fn layers(&self) -> Option<HashSet<SpecRepoLayer>> {
        self.layers.clone().map(|layer_names| {
            layer_names
                .keys()
                .map(|name| match name.as_str() {
                    "." => SpecRepoLayer::Root,
                    _ => SpecRepoLayer::Named(name.clone()),
                })
                .collect()
        })
    }

    pub fn resolved_layers(&self, repo_path: PathBuf) -> Option<HashSet<Layer>> {
        let repo_dir_name = repo_path.file_name().unwrap().to_str().unwrap().to_string();
        self.layers().map(|mut layers| {
            layers
                .drain()
                .map(|layer| match layer {
                    SpecRepoLayer::Root => Layer {
                        name: repo_dir_name.clone(),
                        path: repo_path.clone(),
                    },
                    SpecRepoLayer::Named(name) => Layer {
                        name: name.clone(),
                        path: repo_path.join(name),
                    },
                })
                .collect()
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SpecRemote {
    pub(crate) url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActiveSpec {
    pub(crate) spec: Spec,

    pub(crate) from_stream: String,

    #[serde(skip)]
    pub(crate) weak_stream: Weak<Stream>,
}

impl From<Spec> for ActiveSpec {
    fn from(spec: Spec) -> Self {
        ActiveSpec {
            from_stream: spec.weak_stream.upgrade().unwrap().name().clone(),
            weak_stream: spec.weak_stream.clone(),
            spec,
        }
    }
}

impl ActiveSpec {
    pub fn name(&self) -> String {
        self.spec.header.name.clone()
    }

    pub fn stream(&self) -> Weak<Stream> {
        self.weak_stream.clone()
    }
}
