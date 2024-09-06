use std::collections::HashMap;
use std::fs::File;
use std::io::BufRead;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::time::SystemTime;
use std::{cmp, io};
use std::ffi::OsStr;
use async_trait::async_trait;
use clap::value_parser;
use color_eyre::Help;
use console::Style;
use indicatif::MultiProgress;
use itertools::Itertools;
use multi_index_map::MultiIndexMap;
use serde::Deserialize;
use serde_with::TimestampSecondsWithFrac;
use serde_with::{serde_as, DisplayFromStr};
use time::macros::format_description;
use time::OffsetDateTime;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio_stream::wrappers::LinesStream;
use tokio_stream::{Stream, StreamExt, StreamMap};
use tracing::info_span;
use walkdir::{DirEntry, WalkDir};

use crate::commands::SubcommandRunner;
use crate::core::tool_context::require_tool_context;
use crate::errors::YbResult;
use crate::util::indicatif::MultiProgressHelpers;
use crate::Config;

/// Re-execute the most recent task(s) (by default) that BitBake ran.
#[derive(Debug, clap::Parser)]
#[clap(verbatim_doc_comment, visible_aliases = & ["twice_bake", "twicebake", "tb"])]
pub struct TwiceBakeCommand {
    /// By default, this command does a dry-run. Pass this flag to actually run tasks.
    #[clap(long, short)]
    execute: bool,

    /// Use the Nth most recent invocation of bitbake, rather than the most recent.
    #[clap(long, short, default_value_t = 1, id = "N", value_parser = value_parser!(u8).range(1..))]
    previous: u8,

    // /// By default, this command only executes tasks if they all belong to the same recipe (PN).
    // /// Pass this flag to disable that sanity check.
    // force: bool,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum TaskOutcome {
    Fail,
    Success,
}

#[serde_as]
#[derive(Deserialize, MultiIndexMap, Debug)]
#[multi_index_derive(Debug)]
struct YbSupportHistoryFile {
    #[serde(rename = "PN")]
    #[multi_index(hashed_non_unique)]
    pn: String,
    #[serde(rename = "PV")]
    pv: String,
    #[serde(rename = "T")]
    t: PathBuf,
    #[serde(rename = "WORKDIR")]
    workdir: PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    class_version: u32,

    #[serde_as(as = "Option<TimestampSecondsWithFrac<f64>>")]
    #[multi_index(ordered_non_unique)]
    end_time: Option<SystemTime>,

    #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
    #[multi_index(ordered_non_unique)]
    start_time: SystemTime,

    log_file: String,
    mc: String,
    outcome: Option<TaskOutcome>,
    postfunc_runfiles: Vec<(String, String)>,
    prefunc_runfiles: Vec<(String, String)>,
    task: String,
    task_file: PathBuf,

    #[serde(rename = "task_runfile")]
    task_runfile_name: String,
}

impl YbSupportHistoryFile {
    fn task_runfile(&self) -> PathBuf {
        self.t.join(&self.task_runfile_name)
    }

    fn is_executable(&self) -> bool {
        let metadata = self.task_runfile().metadata().unwrap();
        metadata.mode() & 0o111 != 0
    }
}

fn find_tmpdirs<P: AsRef<Path>>(build_dir: P) -> impl Iterator<Item = DirEntry> {
    WalkDir::new(build_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .flatten()
        .filter(|e| {
            if !e.path().is_dir() {
                return false;
            }

            let file_name = e.file_name().to_str().unwrap();
            if !(file_name == "tmp" || file_name.starts_with("tmp-")) {
                return false;
            }

            true
        })
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The usage is similar as with the standard library's `Command` type
    let mut child = Command::new("echo")
        .arg("hello")
        .arg("world")
        .spawn()
        .expect("failed to spawn");

    // Await until the command completes
    let status = child.wait().await?;
    println!("the command exited with: {}", status);
    Ok(())
}

async fn launch<P: AsRef<OsStr>>(p: P, mp: MultiProgress) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(p);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn command");

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout_reader: Pin<Box<dyn Stream<Item = io::Result<String>> + Send>> =
        Box::pin(LinesStream::new(tokio::io::BufReader::new(stdout).lines()));

    let stderr_reader: Pin<Box<dyn Stream<Item = io::Result<String>> + Send>> =
        Box::pin(LinesStream::new(tokio::io::BufReader::new(stderr).lines()));

    let mut map = StreamMap::new();
    map.insert("stdout", stdout_reader);
    map.insert("stderr", stderr_reader);

    // Ensure the child process is spawned in the runtime so it can
    // make progress on its own while we await for any output.
    let join_handle = tokio::spawn(async move {
        let status = child
            .wait()
            .await
            .expect("child process encountered an error");

        //println!("child status was: {}", status);
        status
    });

    while let Some(line) = map.next().await {
        let line_str = line.1?;
        mp.suspend(|| println!("{}", Style::from_dotted_str("dim").apply_to(line_str)));
    }

    let status = join_handle.await?;
    println!("Process terminated with status: {}", status);

    status.exit_ok().map_err(|e| e.into())
}

#[async_trait]
impl SubcommandRunner for TwiceBakeCommand {
    async fn run(&self, config: &mut Config, mp: &MultiProgress) -> YbResult<()> {
        let context = require_tool_context(config)?;

        let mut tmpdir_to_history_dir_map = HashMap::new();
        let mut map = MultiIndexYbSupportHistoryFileMap::default();

        // Iterate over each tmpdir
        for tmpdir in find_tmpdirs(context.build_dir()) {
            let span = info_span!("looking at tmpdir", dir = tmpdir.path().to_str());
            let _guard = span.enter();

            // Locate the yb-support build history directory
            let history_dir = tmpdir.path().join("yb-support/history/");
            let maybe_history_dir = history_dir.is_dir().then(|| history_dir.clone());
            tmpdir_to_history_dir_map
                .insert(tmpdir.path().to_path_buf(), maybe_history_dir.clone());
            if maybe_history_dir.is_none() {
                continue;
            }

            // Underneath the history directory, each subdir represents a build.
            let mut history_subdir_walker = WalkDir::new(history_dir)
                .min_depth(1)
                .max_depth(1)
                .into_iter()
                .flatten()
                .filter(|e| e.path().is_dir())
                .sorted_by_key(|e| cmp::Reverse(e.metadata().unwrap().modified().unwrap()));

            let latest_history_dir = history_subdir_walker
                .nth(self.previous as usize - 1)
                .unwrap();

            fn systemtime_strftime<T>(dt: T) -> String
            where
                T: Into<OffsetDateTime>,
            {
                dt.into()
                    .format(format_description!("[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] [offset_hour][offset_minute]"))
                    .unwrap()
            }

            mp.note(format!(
                "selected history dir = {:?}",
                latest_history_dir.path()
            ));

            // Iterate over subdirectories, each of which represents a PF
            let pf_walker = WalkDir::new(latest_history_dir.path())
                .min_depth(1)
                .max_depth(1)
                .into_iter()
                .flatten()
                .filter(|e| e.path().is_dir());

            // Iterate over each task file in the PF subdir
            for pf in pf_walker {
                let task_glob_pattern = format!("{}/do_*.json", pf.path().to_str().unwrap());
                //mp.note(format!("PF={}", pf.file_name().to_str().unwrap()));

                for task_file in glob::glob(&task_glob_pattern).unwrap().flatten() {
                    tracing::info!("file: {:?}", task_file);

                    let file_handle = File::open(&task_file).unwrap();
                    let data: YbSupportHistoryFile = serde_json::from_reader(file_handle)?;
                    map.insert(data);
                }
            }
        }

        if tmpdir_to_history_dir_map.is_empty() {
            return Err(
                eyre::eyre!("didn't find any tmp dirs - have you run a build yet?")
                    .suppress_backtrace(true),
            );
        }

        if tmpdir_to_history_dir_map.iter().all(|t| t.1.is_none()) {
            return Err(eyre::eyre!(
                "found one or more tmp dirs, but not yb-support/history/ directories"
            )
            .suggestion("ensure INHERIT contains yb-support class")
            .suppress_backtrace(true));
        }

        if map.is_empty() {
            mp.warn("no tasks found - did the last bitbake run do anything?");
            return Ok(());
        }

        if !self.execute {
            for entry in map.iter_by_start_time() {
                if !entry.is_executable() {
                    mp.warn(format!("would skip Python task {}:{}", entry.pn, entry.task));
                    continue;
                }

                mp.note(format!("would run {}:{}", entry.pn, entry.task));
            }

            mp.warn("dry run only - pass the -e/--execute flag to run tasks");
        } else {
            enum RunOrSkip {
                Run(PathBuf),
                Skip(PathBuf),
            }

            let mut ps = vec![];
            for entry in map.iter_by_start_time() {
                let p = entry.task_runfile().clone();
                if entry.is_executable() {
                    ps.push(RunOrSkip::Run(p));
                } else {
                    ps.push(RunOrSkip::Skip(p));
                }
            }

            let count = ps.len();
            for (i, p) in ps.iter().enumerate() {
                match p {
                    RunOrSkip::Run(p) => {
                        mp.note(format!("[{}/{}] running {}", i, count, p.to_str().unwrap()));
                        launch(p, mp.clone()).await.unwrap();
                    }
                    RunOrSkip::Skip(p) => {
                        mp.warn(format!("skipping Python task {}", p.to_str().unwrap()));
                    }
                }
            }
        }

        Ok(())
    }
}
