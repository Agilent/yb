use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs, io};

use git2::{Branch, FetchOptions, Repository, StatusOptions};
use maplit::hashset;

use crate::config::Config;
use crate::core::tool_context::{require_tool_context, ToolContext};
use crate::data_model::git::{
    BranchStatus, RemoteTrackingBranch, UpstreamBranchStatus, UpstreamComparison,
};
use crate::data_model::status::{
    find_corresponding_spec_repo_for_repo, ComputedStatus, ComputedStatusEntry, MissingRepo,
    OnDiskNonRepoStatus, OnDiskRepoStatus,
};
use crate::data_model::Layer;
use crate::errors::YbResult;
use crate::spec::SpecRepo;
use crate::status_calculator::bblayers_manager::read_bblayers;
use crate::util::git::{
    check_repository_workdirs_unique, create_revwalk, get_current_local_branch,
    get_remote_for_current_branch, get_remote_tracking_branch, ssh_agent_remote_callbacks,
};
use crate::util::paths::list_subdirectories_sorted;

pub mod bblayers_manager;

pub struct StatusCalculatorOptions<'cfg> {
    config: &'cfg Config,
    no_fetch: bool,
    log: bool,
}

impl<'cfg> StatusCalculatorOptions<'cfg> {
    pub fn new(config: &'cfg Config, no_fetch: bool, log: bool) -> Self {
        Self {
            config,
            no_fetch,
            log,
        }
    }
}

/// Compares a local branch (identified by `local_branch_name`) and remote tracking branch (`tracking_branch`)
/// to determine if the former is up-to-date, ahead, behind, or diverged from the latter.
pub fn compare_branch_to_remote_tracking_branch(
    repo: &Repository,
    local_branch_name: String,
    tracking_branch: &RemoteTrackingBranch,
) -> YbResult<UpstreamComparison> {
    let remote_branch_name = tracking_branch.to_string();
    let ahead_count = create_revwalk(
        repo,
        &format!("{1}..{0}", local_branch_name, remote_branch_name),
    )?
    .count();
    let behind_count = create_revwalk(
        repo,
        &format!("{0}..{1}", local_branch_name, remote_branch_name),
    )?
    .count();
    Ok(match (ahead_count > 0, behind_count > 0) {
        (true, true) => UpstreamComparison::Diverged {
            ahead: ahead_count,
            behind: behind_count,
        },
        (true, false) => UpstreamComparison::Ahead(ahead_count),
        (false, true) => UpstreamComparison::Behind(behind_count),
        _ => UpstreamComparison::UpToDate,
    })
}

/// Compares a local branch (`local_branch`) to its remote tracking branch via `compare_branch_to_remote_tracking_branch`.
/// Returns None if local branch has no remote tracking branch.
fn compare_branch_to_upstream(
    repo: &Repository,
    local_branch: &Branch,
) -> YbResult<Option<UpstreamBranchStatus>> {
    let local_branch_name = local_branch.name()?.unwrap().to_string();

    get_remote_tracking_branch(local_branch)?
        .map(|tracking_branch| -> YbResult<_> {
            compare_branch_to_remote_tracking_branch(repo, local_branch_name, &tracking_branch).map(
                |comparison| UpstreamBranchStatus {
                    upstream_comparison: comparison,
                    remote_tracking_branch: tracking_branch,
                },
            )
        })
        .transpose()
}

fn looks_like_layer_dir<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    path.is_dir() && path.join("conf").join("layer.conf").is_file()
}

fn detect_layers<P: AsRef<Path>>(start_dir: P) -> YbResult<HashSet<Layer>> {
    // TODO depth?

    let start_dir = start_dir.as_ref();
    let layers;
    if looks_like_layer_dir(start_dir) {
        // The `start_dir` is itself a single layer
        layers = hashset![Layer {
            path: start_dir.to_path_buf(),
            name: start_dir.file_name().unwrap().to_str().unwrap().to_string(),
        }];
    } else {
        // Detect layers under the path
        layers = fs::read_dir(start_dir)?
            .into_iter()
            .filter_map(|r| r.ok().map(|r| r.path()))
            .filter(|r| looks_like_layer_dir(r))
            .map(|path| Layer {
                path: path.clone(),
                name: path.file_name().unwrap().to_str().unwrap().to_string(),
            })
            .collect();
    }

    Ok(layers)
}

fn compute_repo_status<F>(
    repo: Repository,
    path: &PathBuf,
    options: &mut StatusCalculatorOptions,
    active_spec_repos: &HashMap<String, SpecRepo>,
    c: &mut F,
) -> YbResult<ComputedStatusEntry>
where
    F: FnMut(StatusCalculatorEvent),
{
    // First things first, do a 'git fetch'
    {
        // TODO: fetch all remotes?
        let mut repo_remote = get_remote_for_current_branch(&repo)?;
        // If the current branch is tracking an upstream branch, fetch it to check for updates
        if let Some(remote) = repo_remote.as_mut() {
            if !options.no_fetch {
                c(StatusCalculatorEvent::StartFetch);
                let mut fetch_options = FetchOptions::new();
                fetch_options.remote_callbacks(ssh_agent_remote_callbacks());
                // TODO: this is really slow
                //fetch_options.download_tags(AutotagOption::All);
                remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
                c(StatusCalculatorEvent::FinishFetch);
            }
        }
    }

    // See if we can map the repo to a spec repo
    let spec_repo_status = find_corresponding_spec_repo_for_repo(&repo, active_spec_repos)?;

    let current_branch_status = {
        // TODO: gracefully handle detached HEAD and repos without a tracked branch
        let local_branch = get_current_local_branch(&repo)?;
        let local_branch_name = local_branch.name()?.unwrap().to_string();

        BranchStatus {
            local_branch_name,
            upstream_branch_status: compare_branch_to_upstream(&repo, &local_branch)?,
        }
    };

    // Only run log if enabled and the repo is not diverged
    let commits = if options.log && !current_branch_status.is_diverged() {
        let mut walker = repo.revwalk()?;
        walker.set_sorting(git2::Sort::TOPOLOGICAL)?;
        walker.push_head()?;
        let mut commit_v = vec![];
        for commit in walker.take(5) {
            commit_v.push(commit?);
        }
        Some(commit_v)
    } else {
        None
    };

    let is_workdir_dirty = !repo.statuses(Some(&mut StatusOptions::new()))?.is_empty();

    Ok(ComputedStatusEntry::OnDiskRepo(OnDiskRepoStatus {
        current_branch_status,
        is_workdir_dirty,
        repo,
        corresponding_spec_repo: spec_repo_status,
        path: path.clone(),
        recent_commits: commits,
        layers: detect_layers(&path)?,
    }))
}

pub fn compute_status<F>(mut options: StatusCalculatorOptions, mut c: F) -> YbResult<ComputedStatus>
where
    F: FnMut(StatusCalculatorEvent),
{
    let config = &options.config;
    let arena = toolshed::Arena::new();
    let context = require_tool_context(config, &arena)?;

    let sources_subdirs = match list_subdirectories_sorted(&context.sources_dir())
        .map_err(|e| e.downcast::<io::Error>())
    {
        Ok(result) => result
            .into_iter()
            .map(|subdir| subdir.canonicalize().unwrap())
            .collect::<Vec<_>>(),
        Err(Ok(io_error)) if io_error.kind() == io::ErrorKind::NotFound => {
            // Don't make it an error for the sources directory to be missing
            vec![]
        }
        Err(Ok(io_error)) => {
            eyre::bail!("IO error while enumerating sources directory {}", io_error)
        }
        Err(Err(non_io_error)) => {
            eyre::bail!("error enumerating sources directory {}", non_io_error)
        }
    };

    let active_spec_maybe = match &context {
        ToolContext::Yb(yb_env) => yb_env.active_spec(),
        _ => None,
    };

    let sources_subdirs_with_repo = sources_subdirs
        .iter()
        // TODO: this throws out all errors from `discover`
        .map(|d| (d, Repository::discover(d).ok()))
        .collect::<Vec<_>>();

    let repos = sources_subdirs_with_repo
        .iter()
        .filter_map(|v| v.1.as_ref());

    check_repository_workdirs_unique(repos.clone())?;

    c(StatusCalculatorEvent::Start {
        number_subdirs: sources_subdirs.len() as u64,
        number_repos: repos.count() as u64,
    });

    // If a spec is active, get the expected set of repos, otherwise empty.
    // As we discover spec repos on-disk, we will remove the corresponding entry from this map.
    // What is left is the set of missing spec repos.
    let mut active_spec_repos = active_spec_maybe
        .map(|s| s.spec.repos.clone())
        .unwrap_or_default();

    let mut status_entries: Vec<ComputedStatusEntry> = Vec::with_capacity(sources_subdirs.len());
    for (subdir, repo_maybe) in sources_subdirs_with_repo {
        let subdir_name = subdir.file_name().unwrap().to_str().unwrap().to_string();
        c(StatusCalculatorEvent::StartProcessSubdir {
            dirname: subdir_name.clone(),
        });

        if let Some(repo) = repo_maybe {
            let status =
                compute_repo_status(repo, subdir, &mut options, &active_spec_repos, &mut c)?;
            if let ComputedStatusEntry::OnDiskRepo(OnDiskRepoStatus {
                corresponding_spec_repo: Some(c),
                ..
            }) = &status
            {
                active_spec_repos.remove(&c.spec_repo_name());
            }

            c(StatusCalculatorEvent::SubdirStatusComputed(&status));
            status_entries.push(status);
        } else {
            let status = ComputedStatusEntry::OnDiskNonRepo(OnDiskNonRepoStatus {
                path: subdir.clone(),
            });
            c(StatusCalculatorEvent::SubdirStatusComputed(&status));
            status_entries.push(status);
        }

        c(StatusCalculatorEvent::FinishProcessSubdir);
    }

    let missing_repos = active_spec_repos
        .drain()
        .map(|(name, spec_repo)| MissingRepo { name, spec_repo })
        .collect::<Vec<_>>();

    if !missing_repos.is_empty() {
        c(StatusCalculatorEvent::MissingReposDetected(&missing_repos));
    }

    let bblayers = read_bblayers(&context.build_dir())?;
    let ret = ComputedStatus {
        source_dirs: status_entries,
        enabled_layers: bblayers,
        missing_repos,
        active_spec: active_spec_maybe.cloned(),
        bblayers_path: context.build_dir().join("conf").join("bblayers.conf"),
    };

    c(StatusCalculatorEvent::Finish(&ret));

    Ok(ret)
}

pub enum StatusCalculatorEvent<'a> {
    Start {
        number_repos: u64,
        number_subdirs: u64,
    },
    StartProcessSubdir {
        dirname: String,
    },
    StartFetch,
    StartSubdirOperation {
        operation_name: &'static str,
    },
    FinishFetch,
    SubdirStatusComputed(&'a ComputedStatusEntry),
    FinishProcessSubdir,
    MissingReposDetected(&'a Vec<MissingRepo>),
    Finish(&'a ComputedStatus),
}
