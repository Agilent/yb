use std::path::PathBuf;
use std::process::{Command, Stdio};
use async_trait::async_trait;

use crate::commands::sync::actions::SyncAction;
use crate::data_model::git::RemoteTrackingBranch;
use crate::errors::YbResult;
use crate::spec::SpecRepo;
use crate::util::git::pool_helper::PoolHelper;

#[derive(Debug)]
pub struct ResetGitWorkdirSyncAction {
    repo_path: PathBuf,
}

impl ResetGitWorkdirSyncAction {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }
}

#[async_trait]
impl SyncAction for ResetGitWorkdirSyncAction {
    fn is_force_required(&self) -> bool {
        true
    }

    async fn apply(&self, pool: &PoolHelper) -> YbResult<()> {
        Command::new("git")
            .arg("reset")
            .arg("--hard")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .current_dir(&self.repo_path)
            .output()?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CheckoutBranchSyncAction {
    repo_path: PathBuf,
    branch_name: String,
}

impl CheckoutBranchSyncAction {
    pub fn new(repo_path: PathBuf, branch_name: String) -> Self {
        Self {
            repo_path,
            branch_name,
        }
    }
}

#[async_trait]
impl SyncAction for CheckoutBranchSyncAction {
    fn is_force_required(&self) -> bool {
        false
    }

    async fn apply(&self, pool: &PoolHelper) -> YbResult<()> {
        Command::new("git")
            .arg("checkout")
            .arg(&self.branch_name)
            //.stdout(Stdio::null())
            //.stderr(Stdio::null())
            .current_dir(&self.repo_path)
            .output()?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct FastForwardPullSyncAction {
    repo_path: PathBuf,
    // TODO number of commits?
}

impl FastForwardPullSyncAction {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }
}

#[async_trait]
impl SyncAction for FastForwardPullSyncAction {
    fn is_force_required(&self) -> bool {
        false
    }

    async fn apply(&self, pool: &PoolHelper) -> YbResult<()> {
        Command::new("git")
            .arg("pull")
            .arg("--ff-only")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .current_dir(&self.repo_path)
            .output()?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CreateLocalTrackingBranchSyncAction {
    repo_path: PathBuf,
    local_branch_name: String,
    remote_tracking_branch: RemoteTrackingBranch,
}

impl CreateLocalTrackingBranchSyncAction {
    pub fn new(
        repo_path: PathBuf,
        local_branch_name: String,
        remote_tracking_branch: RemoteTrackingBranch,
    ) -> Self {
        Self {
            repo_path,
            local_branch_name,
            remote_tracking_branch,
        }
    }
}

#[async_trait]
impl SyncAction for CreateLocalTrackingBranchSyncAction {
    fn is_force_required(&self) -> bool {
        false
    }

    async fn apply(&self, pool: &PoolHelper) -> YbResult<()> {
        Command::new("git")
            .arg("checkout")
            .arg("-b")
            .arg(&self.local_branch_name)
            .arg("--track")
            .arg(&self.remote_tracking_branch.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .current_dir(&self.repo_path)
            .output()?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CloneRepoSyncAction {
    dest_repo_path: PathBuf,
    spec_repo: SpecRepo,
}

impl CloneRepoSyncAction {
    pub fn new(dest_repo_path: PathBuf, spec_repo: SpecRepo) -> Self {
        Self {
            dest_repo_path,
            spec_repo,
        }
    }
}

#[async_trait]
impl SyncAction for CloneRepoSyncAction {
    fn is_force_required(&self) -> bool {
        false
    }

    async fn apply(&self, pool: &PoolHelper) -> YbResult<()> {
        pool.clone_in(&self.spec_repo.url, None, Some(self.dest_repo_path.to_str().unwrap().to_string())).await.unwrap().map_err(|e| e.into())

        // Command::new("git")
        //     .arg("clone")
        //     .arg(&self.spec_repo.url)
        //     .arg("-b")
        //     .arg(&self.spec_repo.refspec)
        //     .arg(&self.dest_repo_path)
        //     .stdout(Stdio::null())
        //     .stderr(Stdio::null())
        //     .output()?;
        // Ok(())
    }
}
