use assert_cmd::Command;
use core::fmt;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::path::{Path, PathBuf};

use crate::data_model::git::{
    BranchStatus, LocalTrackingBranch, LocalTrackingBranchWithUpstreamComparison,
    RemoteTrackingBranch,
};
use crate::data_model::Layer;
use git2::{Branch, BranchType, Oid, Repository};
use itertools::Itertools;
use serde::Serialize;
use tempfile::TempDir;

use crate::errors::YbResult;
use crate::spec::{ActiveSpec, SpecRepo};
use crate::status_calculator::{compare_branch_to_remote_tracking_branch, StatusCalculatorEvent};

use crate::util::git::get_remote_tracking_branch;

/// The status of the Yocto environment
#[derive(Debug, Serialize)]
pub struct ComputedStatus {
    pub(crate) source_dirs: Vec<ComputedStatusEntry>,
    pub(crate) enabled_layers: HashSet<Layer>,
    pub(crate) missing_repos: Vec<MissingRepo>,
    pub(crate) active_spec: Option<ActiveSpec>,
    pub(crate) bblayers_path: PathBuf,
}

impl ComputedStatus {
    pub fn active_spec_repos(&self) -> Option<ActiveSpecRepos> {
        let active_spec = self.active_spec.as_ref()?;
        Some(ActiveSpecRepos {
            active_spec_repos: active_spec.spec.repos.iter(),
            source_dirs: &self.source_dirs,
        })
    }

    pub fn spec_requested_layers(&self) -> HashSet<Layer> {
        let mut spec_requested_layers = HashSet::new();
        for entry in &self.source_dirs {
            if let ComputedStatusEntry::OnDiskRepo(repo) = entry {
                if let Some(CorrespondingSpecRepoStatus::RemoteMatch(remote_match_status)) =
                    &repo.corresponding_spec_repo
                {
                    spec_requested_layers.extend(
                        remote_match_status
                            .spec_repo
                            .resolved_layers(repo.path.clone())
                            .unwrap_or_default(),
                    );
                }
            }
        }

        spec_requested_layers
    }

    pub fn missing_bblayers_layers_for_extant_spec_repos(&self) -> HashSet<Layer> {
        // TODO don't clone?
        self.spec_requested_layers()
            .difference(&self.enabled_layers)
            .cloned()
            .collect()
    }

    pub fn extraneous_bblayers_layers(&self) -> HashSet<Layer> {
        self.enabled_layers
            .difference(&self.spec_requested_layers())
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
pub enum ActiveSpecRepoStatus<'a> {
    Missing(&'a SpecRepo),
    Extant {
        spec_repo: &'a SpecRepo,
        path: &'a PathBuf,
    },
}

pub struct ActiveSpecRepos<'a> {
    source_dirs: &'a Vec<ComputedStatusEntry>,
    active_spec_repos: std::collections::hash_map::Iter<'a, String, SpecRepo>,
}

impl<'a> Iterator for ActiveSpecRepos<'a> {
    type Item = ActiveSpecRepoStatus<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.active_spec_repos.next().map(|a| {
            if let Some(extant) = self
                .source_dirs
                .iter()
                .find(|entry| (*entry).spec_repo() == Some(a.1))
            {
                ActiveSpecRepoStatus::Extant {
                    spec_repo: a.1,
                    path: extant.path(),
                }
            } else {
                ActiveSpecRepoStatus::Missing(a.1)
            }
        })
    }
}

/// The status of a source directory
#[derive(Debug, Serialize)]
pub enum ComputedStatusEntry {
    /// A repository
    OnDiskRepo(OnDiskRepoStatus),
    /// A directory that is not a repository
    OnDiskNonRepo(OnDiskNonRepoStatus),
}

impl ComputedStatusEntry {
    pub fn path(&self) -> &PathBuf {
        match &self {
            ComputedStatusEntry::OnDiskNonRepo(OnDiskNonRepoStatus { path, .. }) => path,
            ComputedStatusEntry::OnDiskRepo(OnDiskRepoStatus { path, .. }) => path,
        }
    }

    pub fn spec_repo(&self) -> Option<&SpecRepo> {
        match &self {
            ComputedStatusEntry::OnDiskNonRepo(_) => None,
            ComputedStatusEntry::OnDiskRepo(repo) => repo.spec_repo(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OnDiskNonRepoStatus {
    pub(crate) path: PathBuf,
}

#[derive(Serialize)]
pub struct OnDiskRepoStatus {
    /// Repository object
    #[serde(skip)]
    pub repo: Repository,
    /// Path to the directory
    pub path: PathBuf,
    pub is_workdir_dirty: bool,
    #[serde(skip)]
    pub recent_commits: Option<Vec<Oid>>,
    /// Not necessarily the correct branch as far as any active spec is concerned
    pub current_branch_status: BranchStatus,
    /// Status information pertaining to corresponding spec repo, or None if no matching spec repo
    pub corresponding_spec_repo: Option<CorrespondingSpecRepoStatus>,
    /// Layers that were detected inside the repo (via looking for conf/layer.conf)
    pub layers: HashSet<Layer>,
}

impl OnDiskRepoStatus {
    pub fn has_corresponding_spec_repo(&self) -> bool {
        self.corresponding_spec_repo.is_some()
    }

    pub fn spec_repo(&self) -> Option<&SpecRepo> {
        self.corresponding_spec_repo.as_ref().map(|c| c.spec_repo())
    }

    pub fn is_local_branch_tracking_correct_branch(&self) -> bool {
        assert!(
            self.has_corresponding_spec_repo(),
            "need to check for spec repo before using this method!"
        );
        let spec_repo = self.corresponding_spec_repo.as_ref().unwrap();
        match spec_repo {
            CorrespondingSpecRepoStatus::RemoteMatch(remote_match) => remote_match
                .is_local_branch_tracking_correct_branch(
                    &self.current_branch_status.local_branch_name,
                ),
            _ => panic!("need to check for spec repo match type before using this method!"),
        }
    }
}

impl Debug for OnDiskRepoStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnDiskRepo")
            .field("path", &self.path)
            .field("is_workdir_dirty", &self.is_workdir_dirty)
            .field("recent_commits", &self.recent_commits)
            .field("current_branch_status", &self.current_branch_status)
            .field("corresponding_spec_repo", &self.corresponding_spec_repo)
            .field("layers", &self.layers)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub struct MissingRepo {
    pub name: String,
    pub spec_repo: SpecRepo,
}

pub fn find_local_branches_tracking_remote_branch(
    repo: &Repository,
    remote_tracking_branch: &RemoteTrackingBranch,
) -> YbResult<Vec<LocalTrackingBranchWithUpstreamComparison>> {
    let branches: YbResult<Vec<Branch>> = repo
        .branches(Some(BranchType::Local))?
        .map(|branch| -> YbResult<_> { Ok(branch?.0) })
        .collect();

    let filtered = branches?
        .into_iter()
        .filter(|branch| {
            get_remote_tracking_branch(branch)
                .unwrap()
                .map_or(false, |b| b == *remote_tracking_branch)
        })
        .map(|branch| {
            let branch_name = branch.name().unwrap().unwrap().to_string();
            compare_branch_to_remote_tracking_branch(
                repo,
                branch_name.clone(),
                remote_tracking_branch,
            )
            .map(|comparison| LocalTrackingBranchWithUpstreamComparison {
                local_tracking_branch: LocalTrackingBranch {
                    branch_name: branch_name.clone(),
                    remote_tracking_branch: remote_tracking_branch.clone(),
                },
                upstream_comparison: comparison,
            })
        })
        .try_collect()?;

    Ok(filtered)
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct RemoteMatchStatus {
    pub is_extra_remote: bool,
    pub spec_repo: SpecRepo,
    pub spec_repo_name: String,
    pub remote_tracking_branch: RemoteTrackingBranch,
    pub local_branches_tracking_remote: Vec<LocalTrackingBranchWithUpstreamComparison>,
    pub matching_remote_name: String,
}

impl RemoteMatchStatus {
    pub fn is_local_branch_tracking_correct_branch(&self, local_branch_name: &String) -> bool {
        self.local_branches_tracking_remote
            .iter()
            .map(|l| l.local_tracking_branch.branch_name.clone())
            .collect::<Vec<_>>()
            .contains(local_branch_name)
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum CorrespondingSpecRepoStatus {
    RemoteMatch(RemoteMatchStatus),
    RelatedRepo {
        spec_repo: SpecRepo,
        spec_repo_name: String,
    },
}

impl CorrespondingSpecRepoStatus {
    pub fn spec_repo_name(&self) -> String {
        match &self {
            CorrespondingSpecRepoStatus::RemoteMatch(RemoteMatchStatus {
                spec_repo_name, ..
            }) => spec_repo_name.clone(),
            CorrespondingSpecRepoStatus::RelatedRepo { spec_repo_name, .. } => {
                spec_repo_name.clone()
            }
        }
    }

    pub fn spec_repo(&self) -> &SpecRepo {
        match &self {
            CorrespondingSpecRepoStatus::RemoteMatch(remote_match_status) => {
                &remote_match_status.spec_repo
            }
            CorrespondingSpecRepoStatus::RelatedRepo { spec_repo, .. } => spec_repo,
        }
    }
}

// TODO introduce type for return
pub fn enumerate_repo_remotes(repo: &Repository) -> YbResult<HashMap<String, String>> {
    let remote_names = repo.remotes()?;

    let remotes: Vec<_> = remote_names
        .iter()
        .map(|remote_name| -> YbResult<_> {
            let remote_name = remote_name.unwrap(); // assume utf-8
            Ok((remote_name, repo.find_remote(remote_name)?))
        })
        .try_collect()?;

    Ok(remotes
        .into_iter()
        .filter_map(|(remote_name, remote)| {
            remote
                .url()
                .map(|remote_url| (remote_name.to_string(), remote_url.to_string()))
        })
        .collect())
}

pub fn enumerate_revisions<P: AsRef<Path>>(repo_path: P) -> YbResult<HashSet<String>> {
    // git rev-list --all --full-history
    let revs = Command::new("git")
        .arg("rev-list")
        .arg("--all")
        .arg("--full-history")
        .current_dir(repo_path)
        .output()?
        .stdout;

    Ok(std::str::from_utf8(revs.as_slice())
        .unwrap()
        .lines()
        .map(String::from)
        .collect())
}

pub fn clone_and_enumerate_revisions(spec_repo: &SpecRepo) -> YbResult<HashSet<String>> {
    let tmp = TempDir::new().unwrap();

    let mut cmd = Command::new("git");
    cmd.arg("clone")
        .arg(&spec_repo.url)
        .arg("-b")
        .arg(&spec_repo.refspec)
        .arg(tmp.path());
    cmd.assert().success();

    enumerate_revisions(tmp.path())
}

/// For the on-disk repository `repo`, try to find corresponding spec repo using these methods:
///     1. Check if the repos share a remote (either primary or extra)
///     2. See if the on-disk repo and the spec repo remote has any common commits (by cloning the
///         latter to a temporary directory)
/// TODO document does not validate refspec
pub fn find_corresponding_spec_repo_for_repo<F>(
    repo: &Repository,
    spec_repos: &HashMap<String, SpecRepo>,
    c: &mut F,
) -> YbResult<Option<CorrespondingSpecRepoStatus>>
where
    F: FnMut(StatusCalculatorEvent),
{
    let repo_subdir_name = repo
        .path()
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let remote_names_with_urls = enumerate_repo_remotes(repo)?;

    // Iterate through each spec repo
    for (spec_repo_subdir_name, spec_repo) in spec_repos {
        // Iterate through each of the on-disk repo's remotes
        for (remote_name, remote_url) in &remote_names_with_urls {
            let tracking_branch = RemoteTrackingBranch {
                branch_name: spec_repo.refspec.clone(),
                remote_name: remote_name.clone(),
            };

            if *remote_url == spec_repo.url {
                // The remote URL exactly matches what the spec expects
                return Ok(Some(CorrespondingSpecRepoStatus::RemoteMatch(
                    RemoteMatchStatus {
                        spec_repo: spec_repo.clone(),
                        spec_repo_name: spec_repo_subdir_name.clone(),
                        is_extra_remote: false,
                        local_branches_tracking_remote: find_local_branches_tracking_remote_branch(
                            repo,
                            &tracking_branch,
                        )?,
                        remote_tracking_branch: tracking_branch,
                        matching_remote_name: remote_name.clone(),
                    },
                )));
            }
        }

        // Consider extra remotes
        for (remote_name, remote_url) in &remote_names_with_urls {
            let tracking_branch = RemoteTrackingBranch {
                branch_name: spec_repo.refspec.clone(),
                remote_name: remote_name.clone(),
            };

            if spec_repo
                .extra_remotes
                .iter()
                .any(|(_, extra_remote)| *remote_url == extra_remote.url)
            {
                // The remote URL matches one of the extra remotes in the spec
                // TODO revisit assertion
                assert_eq!(
                    repo_subdir_name, spec_repo_subdir_name,
                    "TODO revisit assertion"
                );
                return Ok(Some(CorrespondingSpecRepoStatus::RemoteMatch(
                    RemoteMatchStatus {
                        spec_repo: spec_repo.clone(),
                        spec_repo_name: spec_repo_subdir_name.clone(),
                        is_extra_remote: true,
                        local_branches_tracking_remote: find_local_branches_tracking_remote_branch(
                            repo,
                            &tracking_branch,
                        )?,
                        remote_tracking_branch: tracking_branch,
                        matching_remote_name: remote_name.clone(),
                    },
                )));
            }
        }
    }

    // Make another pass through spec repos to look for related repos
    for (spec_repo_subdir_name, spec_repo) in spec_repos {
        if repo_subdir_name == spec_repo_subdir_name {
            let op = format!("checking possible upstream {}", spec_repo.url);
            c(StatusCalculatorEvent::StartSubdirOperation { operation_name: op });
            let spec_repo_revs = clone_and_enumerate_revisions(spec_repo)?;
            let on_disk_revs = enumerate_revisions(repo.path())?;
            c(StatusCalculatorEvent::StartSubdirOperation {
                operation_name: "".into(),
            });

            if spec_repo_revs.is_disjoint(&on_disk_revs) {
                continue;
            }

            return Ok(Some(CorrespondingSpecRepoStatus::RelatedRepo {
                spec_repo: spec_repo.clone(),
                spec_repo_name: spec_repo_subdir_name.clone(),
            }));
        }
    }

    Ok(None)
}
