use crate::data_model::Layer;
use crate::errors::YbResult;
use crate::stream_db::StreamKey;
use color_eyre::Help;
use eyre::Report;
use itertools::Itertools;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

const SPEC_FORMAT_VERSION: u32 = 1;

const fn default_format_version() -> u32 {
    SPEC_FORMAT_VERSION
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Spec {
    header: SpecHeader,

    #[serde(deserialize_with = "deserialize_and_inject_key")]
    pub(crate) repos: HashMap<String, SpecRepo>,

    #[serde(skip)]
    pub(crate) stream_key: StreamKey,
}

impl PartialEq for Spec {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header && self.repos == other.repos
    }
}

impl Eq for Spec {}

pub fn deserialize_and_inject_key<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, SpecRepo>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MapVisitor;

    impl<'de> Visitor<'de> for MapVisitor {
        type Value = HashMap<String, SpecRepo>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a map of records")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut values = HashMap::new();

            while let Some((key, mut record)) = map.next_entry::<String, SpecRepo>()? {
                // Inject key as name
                record.name = key.clone();
                values.insert(key, record);
            }

            Ok(values)
        }
    }

    deserializer.deserialize_map(MapVisitor)
}

impl Spec {
    pub fn load(path: &Path, stream_key: StreamKey) -> YbResult<Self> {
        let f = File::open(path)?;
        let mut ret = serde_yaml::from_reader::<_, Self>(f).map_err(Report::from)?;
        ret.stream_key = stream_key;

        // Validation: ensure no overlap between repo URLs
        let mut urls_to_repos: HashMap<&String, HashSet<&String>> = HashMap::new();
        for (repo_name, spec_repo) in &ret.repos {
            let entry = urls_to_repos.entry(&spec_repo.url).or_default();
            entry.insert(repo_name);

            for (repo_name, spec_remote) in &spec_repo.extra_remotes {
                let entry = urls_to_repos.entry(&spec_remote.url).or_default();
                entry.insert(repo_name);
            }
        }

        for (url, repo_names) in urls_to_repos {
            if repo_names.len() > 1 {
                return Err(eyre::eyre!(
                    "URL {} corresponds to more than one spec repo: {}",
                    url,
                    repo_names.into_iter().join(", ")
                )
                .suppress_backtrace(true));
            }
        }

        // Validation: a refspec that's all-hex but not a full-length hash is almost
        // certainly a typo'd commit hash rather than a real branch name.
        for spec_repo in ret.repos.values() {
            if looks_like_truncated_hash(&spec_repo.refspec) {
                return Err(eyre::eyre!(
                    "repo {}: refspec '{}' looks like a truncated commit hash - yb requires \
                     the full 40 (or 64) character hash to pin a commit",
                    spec_repo.name,
                    spec_repo.refspec
                )
                .suppress_backtrace(true));
            }
        }

        Ok(ret)
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

/// Classification of a `refspec` string: either a branch name or a full commit hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefSpecKind {
    Branch(String),
    /// Full hex commit hash (40 chars for SHA-1, 64 for SHA-256), already validated.
    Commit(String),
}

impl RefSpecKind {
    /// A `refspec` is only treated as a pinned commit when it is a full-length hex
    /// string - this mirrors how `git checkout`/`git rev-parse` already disambiguate
    /// commit-ish vs ref-ish arguments.
    pub fn classify(refspec: &str) -> RefSpecKind {
        if is_full_commit_hash(refspec) {
            RefSpecKind::Commit(refspec.to_string())
        } else {
            RefSpecKind::Branch(refspec.to_string())
        }
    }
}

fn is_full_commit_hash(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// A refspec that's all-hex but not a valid full-length hash is neither a plausible
/// branch name nor a usable pin - almost certainly a typo'd commit hash.
fn looks_like_truncated_hash(s: &str) -> bool {
    s.len() >= 6 && s.len() < 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SpecRepo {
    #[serde(skip_deserializing)]
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) refspec: String,
    #[serde(
        rename = "extra-remotes",
        default,
        deserialize_with = "deserialize_null_default"
    )]
    pub(crate) extra_remotes: HashMap<String, SpecRemote>,

    #[serde(
        rename = "obsolete-remotes",
        default,
        deserialize_with = "deserialize_null_default"
    )]
    pub(crate) obsolete_remotes: HashMap<String, SpecRemote>,

    // each entry is a layer name
    pub(crate) layers: Option<HashMap<String, ()>>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SpecRepoLayer {
    Root,
    Named(String),
}

impl SpecRepo {
    pub fn refspec_kind(&self) -> RefSpecKind {
        RefSpecKind::classify(&self.refspec)
    }

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
    pub(crate) stream_key: StreamKey,
}

impl ActiveSpec {
    pub fn name(&self) -> String {
        self.spec.header.name.clone()
    }

    pub fn stream_key(&self) -> StreamKey {
        self.stream_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refspec_kind_classifies_full_hashes_as_commits() {
        let sha1 = "a".repeat(40);
        let sha256 = "b".repeat(64);
        assert_eq!(
            RefSpecKind::classify(&sha1),
            RefSpecKind::Commit(sha1.clone())
        );
        assert_eq!(
            RefSpecKind::classify(&sha256),
            RefSpecKind::Commit(sha256.clone())
        );

        let mixed_case = "ABCDEF0123456789abcdef0123456789ABCDEF01";
        assert_eq!(
            RefSpecKind::classify(mixed_case),
            RefSpecKind::Commit(mixed_case.to_string())
        );
    }

    #[test]
    fn refspec_kind_classifies_branch_names_as_branches() {
        for branch in ["scarthgap", "rel-v2024.2", "scarthgap/rust", "master"] {
            assert_eq!(
                RefSpecKind::classify(branch),
                RefSpecKind::Branch(branch.to_string())
            );
        }
    }

    #[test]
    fn looks_like_truncated_hash_rejects_short_hex_strings_only() {
        assert!(looks_like_truncated_hash("a1b2c3d"));
        assert!(!looks_like_truncated_hash("scarthgap"));
        assert!(!looks_like_truncated_hash(&"a".repeat(40)));
    }

    #[test]
    fn load_rejects_truncated_hash_refspec() {
        let dir = tempfile::tempdir().unwrap();
        let spec_path = dir.path().join("spec.yaml");
        std::fs::write(
            &spec_path,
            indoc::indoc! {r#"
                header:
                    name: "test"
                repos:
                    some-repo:
                        url: "https://example.com/some-repo.git"
                        refspec: "a1b2c3d"
            "#},
        )
        .unwrap();

        let err = Spec::load(&spec_path, StreamKey::default()).unwrap_err();
        assert!(err.to_string().contains("truncated commit hash"));
    }
}
