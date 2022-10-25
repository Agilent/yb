use async_trait::async_trait;
use std::time::Duration;

use console::{Emoji, Style};
use git2::StatusOptions;
use indicatif::{MultiProgress, ProgressBar, ProgressFinish, ProgressStyle};

use crate::commands::SubcommandRunner;
use crate::data_model::git::{BranchStatus, UpstreamComparison};
use crate::data_model::status::{ComputedStatusEntry, CorrespondingSpecRepoStatus};
use crate::errors::YbResult;
use crate::status_calculator::{compute_status, StatusCalculatorEvent, StatusCalculatorOptions};
use crate::ui_ops::update_stream::{ui_op_update_stream, UiUpdateStreamOptions};
use crate::util::git::format_short_statuses;
use crate::util::indicatif::{IndicatifHelpers, MultiProgressHelpers};
use crate::Config;

#[derive(Debug, clap::Parser)]
pub struct StatusCommand {
    /// Don't run 'git fetch' on source dirs
    #[clap(name = "no-fetch", short, long)]
    flag_no_fetch: bool,

    /// Show the most recent 5 'git log' entries
    #[clap(name = "log", short, long)]
    flag_log: bool,

    /// Exclude from the output source dirs for which no differences/suggestions are detected
    #[clap(name = "skip-unremarkable", short, long)]
    skip_unremarkable: bool,
}

struct UpstreamStatusMessage {
    pub message: String,
    pub style: Option<Style>,
}

fn format_upstream_status_message(branch_status: &BranchStatus) -> Option<UpstreamStatusMessage> {
    let behind_symbol = Style::from_dotted_str("bold.yellow").apply_to(Emoji("↙", ""));
    let ahead_symbol = Style::from_dotted_str("bold.bright.magenta").apply_to(Emoji("↗", ""));
    let mut branch_status_color = None;

    let status = branch_status.upstream_branch_status.as_ref()?;

    let remote_tracking_branch_name = &status.remote_tracking_branch.to_string();

    let message = match status.upstream_comparison {
        UpstreamComparison::Diverged { behind, ahead } => {
            let style = Style::from_dotted_str("red.bold");
            let ret = format!(
                "{}: {} {} commits behind '{}', {} {} ahead",
                style.apply_to("diverged"),
                behind_symbol,
                behind,
                remote_tracking_branch_name,
                ahead_symbol,
                ahead
            );
            branch_status_color = Some(style);
            ret
        }
        UpstreamComparison::Behind(behind) => {
            branch_status_color = Some(Style::from_dotted_str("yellow.bold"));
            format!(
                "{} {} commits behind '{}'",
                behind_symbol, behind, remote_tracking_branch_name
            )
        }
        UpstreamComparison::Ahead(ahead) => {
            branch_status_color = Some(Style::from_dotted_str("magenta.bright.bold"));
            format!(
                "{} {} commits ahead of '{}'",
                ahead_symbol, ahead, remote_tracking_branch_name
            )
        }

        UpstreamComparison::UpToDate => {
            format!("up to date with '{}'", remote_tracking_branch_name)
        }
    };

    Some(UpstreamStatusMessage {
        message,
        style: branch_status_color,
    })
}

use crate::ui_ops::check_broken_streams::{
    ui_op_check_broken_streams, UiCheckBrokenStreamsOptions,
};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SPINNER_STRINGS: Vec<String> = {
        let mut chars = "⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈"
            .chars()
            .map(|c| format!("{} ", c))
            .collect::<Vec<_>>();
        chars.push(String::new());
        chars
    };
}

#[async_trait]
impl SubcommandRunner for StatusCommand {
    async fn run(&self, config: &mut Config, mp: &MultiProgress) -> YbResult<()> {
        ui_op_check_broken_streams(UiCheckBrokenStreamsOptions::new(config, mp))?;

        // Check the stream (if active) for updates
        let update_stream_opts = UiUpdateStreamOptions::new(config, mp);
        ui_op_update_stream(update_stream_opts)?;

        let status_calculator_options =
            StatusCalculatorOptions::new(config, self.flag_no_fetch, self.flag_log);

        let mut overall_progress: Option<ProgressBar> = None;
        let mut subdir_spinner: Option<ProgressBar> = None;

        let mut subdir_lines: Vec<ProgressBar> = vec![];

        let status = compute_status(status_calculator_options, |event| {
            match event {
                StatusCalculatorEvent::Start { number_subdirs, .. } => {
                    overall_progress.replace(
                        mp.add(
                            ProgressBar::new(number_subdirs)
                                .with_message("checking source directories")
                                .with_style(
                                    ProgressStyle::with_template(
                                        "\n{msg} [{wide_bar}] {pos}/{len}",
                                    )
                                    .unwrap()
                                    .progress_chars("##-"),
                                ),
                        ),
                    );

                    overall_progress.as_ref().unwrap().tick();
                }
                StatusCalculatorEvent::StartProcessSubdir { dirname } => {
                    subdir_lines.clear();

                    subdir_spinner.replace(
                        mp.insert_before(
                            overall_progress.as_ref().unwrap(),
                            ProgressBar::new_spinner()
                                .with_style(
                                    ProgressStyle::with_template(
                                        "{spinner:.yellow.bright}{msg}: {prefix}",
                                    )
                                    .unwrap()
                                    .tick_strings(
                                        &SPINNER_STRINGS
                                            .iter()
                                            .map(|s| s.as_str())
                                            .collect::<Vec<_>>(),
                                    ),
                                )
                                .with_finish(ProgressFinish::AndLeave)
                                .with_message(
                                    Style::from_dotted_str("blue.bold")
                                        .apply_to(dirname)
                                        .to_string(),
                                ),
                        ),
                    );

                    subdir_spinner
                        .as_ref()
                        .unwrap()
                        .enable_steady_tick(Duration::from_millis(50));
                    subdir_spinner.as_ref().unwrap().tick();
                    subdir_lines.push(subdir_spinner.as_ref().unwrap().clone());

                    subdir_lines.push(mp.println_before(subdir_spinner.as_ref().unwrap(), " "));
                }
                StatusCalculatorEvent::StartFetch => {
                    subdir_spinner.as_ref().unwrap().set_prefix("fetching...")
                }
                StatusCalculatorEvent::FinishFetch => {
                    subdir_spinner.as_ref().unwrap().set_prefix("")
                }
                StatusCalculatorEvent::StartSubdirOperation { operation_name } => {
                    subdir_spinner.as_ref().unwrap().set_prefix(operation_name)
                }
                StatusCalculatorEvent::SubdirStatusComputed(status) => {
                    match status {
                        ComputedStatusEntry::OnDiskRepo(repo_status) => {
                            let on_branch_message = mp.println_after(
                                subdir_spinner.as_ref().unwrap(),
                                format!(
                                    "\ton branch '{}'",
                                    &repo_status.current_branch_status.local_branch_name
                                ),
                            );
                            subdir_lines.push(on_branch_message.clone());

                            let branch_message = mp.println_after(&on_branch_message, "");
                            subdir_lines.push(branch_message.clone());
                            let mut branch_status_color = None;

                            // Report difference to upstream branch (if on branch and tracking an upstream)
                            if let Some(current_branch_status_message) =
                                format_upstream_status_message(&repo_status.current_branch_status)
                            {
                                branch_message.set_message(format!(
                                    "\t{}",
                                    current_branch_status_message.message
                                ));
                                branch_status_color = current_branch_status_message.style;
                            }

                            if let Some(corresponding_spec_repo_status) =
                                &repo_status.corresponding_spec_repo
                            {
                                let corresponding_spec_repo_message =
                                    mp.println_after(&branch_message, "");
                                subdir_lines.push(corresponding_spec_repo_message.clone());

                                match &corresponding_spec_repo_status {
                                    CorrespondingSpecRepoStatus::RemoteMatch(
                                        remote_match_status,
                                    ) => {
                                        if !repo_status.is_local_branch_tracking_correct_branch() {
                                            if !remote_match_status
                                                .local_branches_tracking_remote
                                                .is_empty()
                                            {
                                                corresponding_spec_repo_message.set_message(
                                                        Style::new().red().apply_to(format!(
                                                            "\tshould be on a branch tracking '{}', such as:",
                                                            remote_match_status.remote_tracking_branch.to_string()
                                                        )).to_string(),
                                                    );

                                                for branch in &remote_match_status
                                                    .local_branches_tracking_remote
                                                {
                                                    let last_message = subdir_lines.last().unwrap();
                                                    subdir_lines.push(
                                                        mp.println_after(
                                                            last_message,
                                                            Style::new()
                                                                .red()
                                                                .apply_to(format!(
                                                                    "\t\t{}",
                                                                    branch
                                                                        .local_tracking_branch
                                                                        .branch_name
                                                                ))
                                                                .to_string(),
                                                        ),
                                                    );
                                                }
                                            } else {
                                                corresponding_spec_repo_message.set_message(
                                                    Style::new()
                                                        .red()
                                                        .apply_to(format!(
                                                            "\tshould be on a branch tracking '{}'",
                                                            remote_match_status
                                                                .remote_tracking_branch
                                                                .to_string()
                                                        ))
                                                        .to_string(),
                                                );
                                            }

                                            branch_status_color =
                                                Some(Style::from_dotted_str("red.bold"));
                                        }
                                    }
                                    CorrespondingSpecRepoStatus::PossibleMatch { .. } => {
                                        corresponding_spec_repo_message.set_message(
                                                Style::new().red().on_white().apply_to("\tthis directory has same name of a spec repo, but isn't tracking any expected remote?".to_string()).to_string(),
                                            );

                                        branch_status_color =
                                            Some(Style::from_dotted_str("red.bold"));
                                    }
                                }
                            }

                            if let Some(commit_ids) = &repo_status.recent_commits {
                                for id in commit_ids {
                                    let commit = repo_status.repo.find_commit(*id).unwrap(); // TODO use YbResult
                                    let oneline = format!(
                                        "\t{} {}",
                                        Style::default().yellow().apply_to(
                                            commit
                                                .as_object()
                                                .short_id()
                                                .unwrap()
                                                .as_str()
                                                .unwrap()
                                        ),
                                        commit.summary().unwrap()
                                    );

                                    let last_message = subdir_lines.last().unwrap();
                                    subdir_lines.push(mp.println_after(last_message, oneline));
                                }
                            }

                            let mut opts = StatusOptions::new();
                            let statuses = repo_status.repo.statuses(Some(&mut opts)).unwrap(); // TODO YbResult
                            if !statuses.is_empty() {
                                branch_status_color = Some(Style::from_dotted_str("red.bold"));
                            }

                            for short_status in format_short_statuses(&repo_status.repo, &statuses)
                            {
                                let last_message = subdir_lines.last().unwrap();
                                subdir_lines.push(mp.println_after(
                                    last_message,
                                    Style::default().red().apply_to(short_status).to_string(),
                                ));
                            }

                            // Re-color the subdir spinner label if there is a status to be reported
                            if let Some(branch_status_style) = branch_status_color {
                                subdir_spinner
                                    .as_ref()
                                    .unwrap()
                                    .restyle_message(branch_status_style);
                            } else if self.skip_unremarkable {
                                for line in subdir_lines.drain(..) {
                                    line.finish_and_clear();
                                }
                            }
                        }
                        // TODO
                        _ => {}
                    }
                }
                StatusCalculatorEvent::FinishProcessSubdir => {
                    overall_progress.as_ref().unwrap().inc(1);
                    subdir_spinner.take();
                }
                StatusCalculatorEvent::MissingReposDetected(missing_repos) => {
                    if missing_repos.is_empty() {
                        return;
                    }

                    mp.println_before(overall_progress.as_ref().unwrap(), " ");
                    mp.println_before(
                        overall_progress.as_ref().unwrap(),
                        format!(
                            "==> {} <==",
                            Style::from_dotted_str("red.on_white.bold")
                                .apply_to("missing repositories detected!")
                        ),
                    );

                    for missing in missing_repos {
                        mp.println_after(
                            overall_progress.as_ref().unwrap(),
                            format!(
                                "{}: {}, remote: {}",
                                Style::from_dotted_str("bold").apply_to("missing repo"),
                                missing.name,
                                missing.spec_repo.url
                            ),
                        );
                    }
                }
                _ => {}
            }
        })?;

        if config.porcelain {
            let json = serde_json::to_string_pretty(&status);
            println!("{}", json?);
        }

        Ok(())
    }
}
