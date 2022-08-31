use std::fmt::Debug;

use async_trait::async_trait;

use console::Style;
use futures::StreamExt;
use git2::Repository;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::commands::activate::activate_spec;
use crate::commands::sync::actions::{
    BBLayersEditAction, CheckoutBranchSyncAction, CloneRepoSyncAction,
    CreateLocalTrackingBranchSyncAction, FastForwardPullSyncAction, ModifyBBLayersConfSyncAction,
    ResetGitWorkdirSyncAction, SyncAction,
};
use crate::commands::SubcommandRunner;
use crate::config::Config;
use crate::core::tool_context::require_yb_env;
use crate::data_model::git::{
    determine_optimal_checkout_branch, RemoteTrackingBranch, UpstreamComparison,
};
use crate::data_model::status::{ComputedStatusEntry, CorrespondingSpecRepoStatus};
use crate::errors::YbResult;
use crate::status_calculator::{compute_status, StatusCalculatorEvent, StatusCalculatorOptions};
use crate::ui_ops::check_broken_streams::{
    ui_op_check_broken_streams, UiCheckBrokenStreamsOptions,
};
use crate::ui_ops::update_stream::{ui_op_update_stream, UiUpdateStreamOptions};
use crate::util::git;
use crate::util::indicatif::MultiProgressHelpers;
use concurrent_git_pool::PoolHelper;

mod actions;

/// Analyze the yb environment and determine what needs to be done so that it matches the active spec.
///
/// By default no changes are applied. Pass the --apply/-a flag to make changes.
#[derive(Debug, clap::Parser)]
pub struct SyncCommand {
    /// Activate the given spec before syncing
    spec: Option<String>,

    #[clap(long, short)]
    apply: bool,

    #[clap(long, short)]
    force: bool,

    #[clap(long, short)]
    exact: bool,
}

#[async_trait]
impl SubcommandRunner for SyncCommand {
    async fn run(&self, config: &mut Config, mp: &MultiProgress) -> YbResult<()> {
        ui_op_check_broken_streams(UiCheckBrokenStreamsOptions::new(config, mp))?;

        let arena = toolshed::Arena::new();

        let mut yb_env = require_yb_env(config, &arena)?;

        if let Some(spec_name) = &self.spec {
            // TODO: don't immediately activate. Use current spec and desired spec to better calculate
            // what needs to be done.
            activate_spec(&mut yb_env, spec_name)?;
        }

        if yb_env.active_spec_status().is_none() {
            eyre::bail!("cannot sync unless a spec is active - see the 'yb activate' command");
        }

        let update_stream_opts = UiUpdateStreamOptions::new(config, mp);
        ui_op_update_stream(update_stream_opts)?;

        if self.apply {
            mp.note("gathering status\n\n");
        } else {
            mp.warn("gathering status only - will not modify environment (pass the -a flag to apply changes)\n\n");
        }

        let mut overall_progress: Option<ProgressBar> = None;

        let status_calculator_options = StatusCalculatorOptions::new(config, false, false);
        let status = compute_status(status_calculator_options, |event| match event {
            StatusCalculatorEvent::Start { number_subdirs, .. } => {
                overall_progress.replace(
                    mp.add(
                        ProgressBar::new(number_subdirs)
                            .with_message("checking source directories")
                            .with_style(
                                ProgressStyle::with_template("{msg} [{wide_bar}] {pos}/{len}")
                                    .unwrap()
                                    .progress_chars("##-"),
                            ),
                    ),
                );
            }
            StatusCalculatorEvent::StartProcessSubdir { dirname } => overall_progress
                .as_ref()
                .unwrap()
                .set_message(format!("checking {}", dirname)),
            StatusCalculatorEvent::FinishProcessSubdir => overall_progress.as_ref().unwrap().inc(1),
            _ => {}
        })?;

        drop(overall_progress);

        let mut sync_actions: Vec<Box<dyn SyncAction>> = vec![];

        for status_data in status.source_dirs.iter() {
            let subdir = status_data.path();

            if let ComputedStatusEntry::OnDiskRepo(status_data) = status_data {
                if !status_data.has_corresponding_spec_repo() {
                    println!("skipped {:?}", &subdir);
                    continue;
                }

                if status_data.is_workdir_dirty {
                    sync_actions.push(box ResetGitWorkdirSyncAction::new(status_data.path.clone()))
                }

                match &status_data.corresponding_spec_repo {
                    Some(corresponding_spec_repo_status) => match &corresponding_spec_repo_status {
                        CorrespondingSpecRepoStatus::RelatedRepo { spec_repo, .. } => {
                            println!(
                                "{}",
                                Style::new().red().on_white().apply_to(format!(
                                    "{} shares commits with spec repo {}",
                                    status_data.path.display(),
                                    spec_repo.url
                                ))
                            );
                            panic!();
                        }
                        CorrespondingSpecRepoStatus::RemoteMatch(remote_match) => {
                            if status_data.is_local_branch_tracking_correct_branch() {
                                let upstream_comparison = status_data
                                    .current_branch_status
                                    .upstream_branch_status
                                    .as_ref()
                                    .unwrap()
                                    .upstream_comparison;
                                match upstream_comparison {
                                    UpstreamComparison::UpToDate => {}
                                    UpstreamComparison::Behind(_) => {
                                        sync_actions.push(box FastForwardPullSyncAction::new(
                                            status_data.path.clone(),
                                        ));
                                    }
                                    UpstreamComparison::Ahead(_) => {
                                        let msg = format!("{} is ahead of remote and I don't know what to do about it", status_data.path.display());
                                        mp.error(&msg);
                                        panic!();
                                    }
                                    UpstreamComparison::Diverged { .. } => unimplemented!(),
                                }
                            } else if remote_match.local_branches_tracking_remote.is_empty() {
                                let new_local_branch_name =
                                    determine_local_branch_name_for_checkout(
                                        &status_data.repo,
                                        &remote_match.spec_repo.refspec,
                                    )?;

                                sync_actions.push(box CreateLocalTrackingBranchSyncAction::new(
                                    status_data.path.clone(),
                                    new_local_branch_name.clone(),
                                    RemoteTrackingBranch {
                                        branch_name: remote_match.spec_repo.refspec.clone(),
                                        remote_name: remote_match.matching_remote_name.clone(),
                                    },
                                ));

                                sync_actions.push(box CheckoutBranchSyncAction::new(
                                    status_data.path.clone(),
                                    new_local_branch_name.clone(),
                                ));

                                sync_actions.push(box FastForwardPullSyncAction::new(
                                    status_data.path.clone(),
                                ));
                            } else {
                                let optimal_branch = determine_optimal_checkout_branch(
                                    &remote_match.local_branches_tracking_remote,
                                )
                                .unwrap();

                                sync_actions.push(box CheckoutBranchSyncAction::new(
                                    status_data.path.clone(),
                                    optimal_branch.local_tracking_branch.branch_name.clone(),
                                ));

                                match optimal_branch.upstream_comparison {
                                    UpstreamComparison::UpToDate => {}
                                    UpstreamComparison::Behind(_) => {
                                        sync_actions.push(box FastForwardPullSyncAction::new(
                                            status_data.path.clone(),
                                        ));
                                    }
                                    UpstreamComparison::Ahead(_ahead) => {
                                        // TODO: suggest pushing changes?
                                    }
                                    UpstreamComparison::Diverged { .. } => unimplemented!(),
                                }
                            }
                        }
                    },
                    None => {
                        // TODO
                    }
                }
            }
        }

        for repo in &status.missing_repos {
            let dest = yb_env.sources_dir().join(repo.name.clone());
            sync_actions.push(box CloneRepoSyncAction::new(
                dest.clone(),
                repo.spec_repo.clone(),
            ));

            // TODO add action to temporary clone the repo and precheck that the expected layers
            //  actually exist?
            for layer in repo.spec_repo.resolved_layers(dest) {
                for layer in layer {
                    sync_actions.push(box ModifyBBLayersConfSyncAction::new(
                        layer.path,
                        status.bblayers_path.clone(),
                        BBLayersEditAction::AddLayer,
                    ));
                }
            }
        }

        // This doesn't include layers for missing spec repos - that is handled above
        for layer in status.missing_bblayers_layers_for_extant_spec_repos() {
            sync_actions.push(box ModifyBBLayersConfSyncAction::new(
                layer.path,
                status.bblayers_path.clone(),
                BBLayersEditAction::AddLayer,
            ));
        }

        if self.exact {
            for layer in status.extraneous_bblayers_layers() {
                sync_actions.push(box ModifyBBLayersConfSyncAction::new(
                    layer.path,
                    status.bblayers_path.clone(),
                    BBLayersEditAction::RemoveLayer,
                ));
            }

            // TODO workspace layer
        }

        // TODO backup bblayers.conf before apply

        println!("actions: {:#?}", sync_actions);

        if self.apply {
            if sync_actions.iter().any(|action| action.is_force_required()) && !self.force {
                mp.warn("need to pass --force flag to apply one or more actions");
                panic!();
            }

            println!();
            let progress = mp.add(
                ProgressBar::new(sync_actions.len() as u64).with_style(
                    ProgressStyle::with_template("{msg} [{wide_bar}] {pos}/{len}")
                        .unwrap()
                        .progress_chars("##-"),
                ),
            );
            progress.set_message("applying actions");

            let client = PoolHelper::connect_or_local().await.unwrap();
            for action in sync_actions {
                action.apply(&client).await?;
                progress.inc(1);
            }
        } else if !sync_actions.is_empty() {
            mp.warn("none of these changes have been applied - re-run with -a to apply")
        }

        Ok(())
    }
}

fn determine_local_branch_name_for_checkout(
    repo: &Repository,
    local_branch_name: &str,
) -> YbResult<String> {
    if !git::local_branch_exists(repo, local_branch_name)? {
        return Ok(local_branch_name.to_string());
    }

    // TODO smarter way
    for i in 2..10 {
        let next_try = format!("{}-{}", local_branch_name, i);
        if !git::local_branch_exists(repo, &next_try)? {
            return Ok(next_try);
        }
    }

    unimplemented!("exhausted possible local branch candidates");
}
