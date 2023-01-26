use async_trait::async_trait;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::commands::sync::actions::SyncAction;
use crate::data_model::git::RemoteTrackingBranch;
use crate::errors::YbResult;
use crate::spec::SpecRepo;
use concurrent_git_pool::PoolHelper;

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

    async fn apply(&self, _pool: &PoolHelper) -> YbResult<()> {
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

    async fn apply(&self, _pool: &PoolHelper) -> YbResult<()> {
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

    async fn apply(&self, _pool: &PoolHelper) -> YbResult<()> {
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

    async fn apply(&self, _pool: &PoolHelper) -> YbResult<()> {
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
        pool.clone_in(
            &self.spec_repo.url,
            None,
            Some(self.dest_repo_path.to_str().unwrap().to_string()),
        )
        .await
        .unwrap()?;

        assert_cmd::Command::new("git")
            .current_dir(&self.dest_repo_path)
            .arg("checkout")
            .arg(&self.spec_repo.refspec)
            .assert()
            .success()
            .success();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::sync::actions::{CloneRepoSyncAction, SyncAction};
    use crate::spec::SpecRepo;
    use crate::util::debug_temp_dir::DebugTempDir;
    use assert_cmd::Command;
    use concurrent_git_pool::PoolHelper;

    #[tokio::test]
    async fn clone_action_checks_out_correct_refspec() {
        let dir = DebugTempDir::new().unwrap();
        let dir_path = dir.path().to_path_buf();

        let pool = PoolHelper::connect_or_local().await.unwrap();

        let spec_repo = SpecRepo {
            url: "https://github.com/agherzan/meta-raspberrypi.git".to_string(),
            refspec: "honister".to_string(),
            extra_remotes: Default::default(),
            layers: None,
        };

        let action = CloneRepoSyncAction::new(dir_path.clone(), spec_repo);
        action.apply(&pool).await.unwrap();

        let mut branch_cmd = Command::new("git");
        branch_cmd
            .current_dir(dir_path)
            .arg("branch")
            .arg("--show-current");
        let branch_cmd_output = branch_cmd.output().unwrap();
        let current_branch = std::str::from_utf8(&branch_cmd_output.stdout)
            .unwrap()
            .trim();
        assert_eq!(current_branch, "honister");
    }
}
