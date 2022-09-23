// Some functions below (where noted) are from git2-rs which is dual-licensed MIT and Apache 2.0.
// Those portions are Copyright (c) 2014 Alex Crichton

use std::collections::HashMap;
use std::path::PathBuf;

use crate::data_model::git::RemoteTrackingBranch;
use eyre::eyre;
use git2::ErrorCode::NotFound;
use git2::{
    Branch, BranchType, Cred, ErrorCode, ObjectType, Remote, RemoteCallbacks, Repository, Revwalk,
    SubmoduleIgnore,
};

use crate::errors::YbResult;

pub fn get_current_local_branch(repo: &Repository) -> YbResult<Branch> {
    match repo.head() {
        Ok(head) => Ok(Branch::wrap(head)),
        Err(ref e) if e.code() == ErrorCode::UnbornBranch /*|| e.code() == ErrorCode::NotFound*/ => {
            Err(eyre!("unborn branch"))
        }
        Err(e) => Err(e.into()),
    }
}

pub fn get_current_local_branch_name(repo: &Repository) -> YbResult<String> {
    Ok(get_current_local_branch(repo)?
        .name()?
        .ok_or_else(|| eyre!("couldn't determine shorthand"))?
        .to_string())
}

pub fn get_remote_tracking_branch_for_current_local_branch(
    repo: &Repository,
) -> YbResult<Option<RemoteTrackingBranch>> {
    get_remote_tracking_branch(&get_current_local_branch(repo)?)
}

pub fn get_remote_tracking_branch(branch: &Branch) -> YbResult<Option<RemoteTrackingBranch>> {
    match branch.upstream() {
        Ok(upstream_branch) => {
            let tracking_branch_name = upstream_branch.name()?.unwrap().to_string();
            let tracking_branch_parts = tracking_branch_name.split_once('/').unwrap();
            Ok(Some(RemoteTrackingBranch {
                remote_name: tracking_branch_parts.0.to_string(),
                branch_name: tracking_branch_parts.1.to_string(),
            }))
        }
        Err(err) if err.code() == NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn get_remote_name_for_current_branch(repo: &Repository) -> YbResult<Option<String>> {
    let branch = get_current_local_branch(repo)?;
    // Repository::branch_upstream_remote needs the 'refs/heads/blah'
    let branch_ref_name = branch
        .into_reference()
        .name()
        .ok_or_else(|| eyre!("branch has no name"))?
        .to_string();

    match repo.branch_upstream_remote(&branch_ref_name) {
        Err(ref e) if e.code() == ErrorCode::NotFound => Ok(None),
        Ok(name) => Ok(Some(
            name.as_str()
                .ok_or_else(|| eyre!("couldn't get branch name from reference"))?
                .to_string(),
        )),
        Err(e) => Err(e.into()),
    }
}

pub fn get_remote_for_current_branch(repo: &Repository) -> YbResult<Option<Remote>> {
    get_remote_name_for_current_branch(repo)?
        .map(|n| repo.find_remote(&n))
        .transpose()
        .map_err(|e| e.into())
}

// Adapted from libgit2-rs
pub fn create_revwalk<'a>(repo: &'a Repository, commit: &str) -> YbResult<Revwalk<'a>> {
    let mut revwalk = repo.revwalk()?;
    let revspec = repo.revparse(commit)?;
    if revspec.mode().contains(git2::RevparseMode::SINGLE) {
        revwalk.push(revspec.from().unwrap().id())?;
    } else {
        let from = revspec.from().unwrap().id();
        let to = revspec.to().unwrap().id();
        revwalk.push(to)?;
        if revspec.mode().contains(git2::RevparseMode::MERGE_BASE) {
            let base = repo.merge_base(from, to)?;
            let o = repo.find_object(base, Some(ObjectType::Commit))?;
            revwalk.push(o.id())?;
        }
        revwalk.hide(from)?;
    }
    Ok(revwalk)
}

// Adapted from libgit2-rs
// This version of the output prefixes each path with two status columns and
// shows submodule status information.
pub fn format_short_statuses(repo: &Repository, statuses: &git2::Statuses) -> Vec<String> {
    let mut ret: Vec<_> = vec![];
    for entry in statuses
        .iter()
        .filter(|e| e.status() != git2::Status::CURRENT)
    {
        let mut istatus = match entry.status() {
            s if s.contains(git2::Status::INDEX_NEW) => 'A',
            s if s.contains(git2::Status::INDEX_MODIFIED) => 'M',
            s if s.contains(git2::Status::INDEX_DELETED) => 'D',
            s if s.contains(git2::Status::INDEX_RENAMED) => 'R',
            s if s.contains(git2::Status::INDEX_TYPECHANGE) => 'T',
            _ => ' ',
        };
        let mut wstatus = match entry.status() {
            s if s.contains(git2::Status::WT_NEW) => {
                if istatus == ' ' {
                    istatus = '?';
                }
                '?'
            }
            s if s.contains(git2::Status::WT_MODIFIED) => 'M',
            s if s.contains(git2::Status::WT_DELETED) => 'D',
            s if s.contains(git2::Status::WT_RENAMED) => 'R',
            s if s.contains(git2::Status::WT_TYPECHANGE) => 'T',
            _ => ' ',
        };

        if entry.status().contains(git2::Status::IGNORED) {
            istatus = '!';
            wstatus = '!';
        }
        if istatus == '?' && wstatus == '?' {
            continue;
        }
        let mut extra = "";

        // A commit in a tree is how submodules are stored, so let's go take a
        // look at its status.
        //
        // TODO: check for GIT_FILEMODE_COMMIT
        let status = entry.index_to_workdir().and_then(|diff| {
            let ignore = SubmoduleIgnore::Unspecified;
            diff.new_file()
                .path_bytes()
                .and_then(|s| std::str::from_utf8(s).ok())
                .and_then(|name| repo.submodule_status(name, ignore).ok())
        });
        if let Some(status) = status {
            if status.contains(git2::SubmoduleStatus::WD_MODIFIED) {
                extra = " (new commits)";
            } else if status.contains(git2::SubmoduleStatus::WD_INDEX_MODIFIED)
                || status.contains(git2::SubmoduleStatus::WD_WD_MODIFIED)
            {
                extra = " (modified content)";
            } else if status.contains(git2::SubmoduleStatus::WD_UNTRACKED) {
                extra = " (untracked content)";
            }
        }

        let (mut a, mut b, mut c) = (None, None, None);
        if let Some(diff) = entry.head_to_index() {
            a = diff.old_file().path();
            b = diff.new_file().path();
        }
        if let Some(diff) = entry.index_to_workdir() {
            a = a.or_else(|| diff.old_file().path());
            b = b.or_else(|| diff.old_file().path());
            c = diff.new_file().path();
        }

        match (istatus, wstatus) {
            ('R', 'R') => ret.push(format!(
                "\tRR {} {} {}{}",
                a.unwrap().display(),
                b.unwrap().display(),
                c.unwrap().display(),
                extra
            )),
            ('R', w) => ret.push(format!(
                "\tR{} {} {}{}",
                w,
                a.unwrap().display(),
                b.unwrap().display(),
                extra
            )),
            (i, 'R') => ret.push(format!(
                "\t{}R {} {}{}",
                i,
                a.unwrap().display(),
                c.unwrap().display(),
                extra
            )),
            (i, w) => ret.push(format!("\t{}{} {}{}", i, w, a.unwrap().display(), extra)),
        };
    }

    for entry in statuses
        .iter()
        .filter(|e| e.status() == git2::Status::WT_NEW)
    {
        ret.push(format!(
            "\t?? {}",
            entry
                .index_to_workdir()
                .unwrap()
                .old_file()
                .path()
                .unwrap()
                .display()
        ));
    }

    ret
}

// Adapted from libgit2-rs
fn fast_forward(
    repo: &Repository,
    lb: &mut git2::Reference,
    rc: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = match lb.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    println!("{}", msg);
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force(),
    ))?;
    Ok(())
}

// Adapted from libgit2-rs
fn normal_merge(
    repo: &Repository,
    local: &git2::AnnotatedCommit,
    remote: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let local_tree = repo.find_commit(local.id())?.tree()?;
    let remote_tree = repo.find_commit(remote.id())?.tree()?;
    let ancestor = repo
        .find_commit(repo.merge_base(local.id(), remote.id())?)?
        .tree()?;
    let mut idx = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

    if idx.has_conflicts() {
        println!("Merge conficts detected...");
        repo.checkout_index(Some(&mut idx), None)?;
        return Ok(());
    }
    let result_tree = repo.find_tree(idx.write_tree_to(repo)?)?;
    // now create the merge commit
    let msg = format!("Merge: {} into {}", remote.id(), local.id());
    let sig = repo.signature()?;
    let local_commit = repo.find_commit(local.id())?;
    let remote_commit = repo.find_commit(remote.id())?;
    // Do our merge commit and set current branch head to that commit.
    let _merge_commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &msg,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;
    // Set working tree to match head.
    repo.checkout_head(None)?;
    Ok(())
}

// Adapted from libgit2-rs
pub fn do_merge<'a>(
    repo: &'a Repository,
    remote_branch: &str,
    fetch_commit: git2::AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    // 1. do a merge analysis
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    // 2. Do the appopriate merge
    if analysis.0.is_fast_forward() {
        //println!("Doing a fast forward");
        // do a fast forward
        let refname = format!("refs/heads/{}", remote_branch);
        match repo.find_reference(&refname) {
            Ok(mut r) => {
                fast_forward(repo, &mut r, &fetch_commit)?;
            }
            Err(_) => {
                // The branch doesn't exist so just set the reference to the
                // commit directly. Usually this is because you are pulling
                // into an empty repository.
                repo.reference(
                    &refname,
                    fetch_commit.id(),
                    true,
                    &format!("Setting {} to {}", remote_branch, fetch_commit.id()),
                )?;
                repo.set_head(&refname)?;
                repo.checkout_head(Some(
                    git2::build::CheckoutBuilder::default()
                        .allow_conflicts(true)
                        .conflict_style_merge(true)
                        .force(),
                ))?;
            }
        };
    } else if analysis.0.is_normal() {
        // do a normal merge
        panic!("merge not yet supported");
        // TODO
        //let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
        //normal_merge(&repo, &head_commit, &fetch_commit)?;
    } else {
        //println!("Nothing to do...");
    }
    Ok(())
}

pub fn check_repository_workdirs_unique<'a, I>(repos: I) -> YbResult<()>
where
    I: Iterator<Item = &'a Repository>,
{
    let mut workdir_to_repo: HashMap<PathBuf, Vec<&Repository>> = HashMap::new();
    for repo in repos {
        let workdir = repo
            .workdir()
            .ok_or_else(|| eyre!("bare repositories not supported"))?;
        let r = workdir_to_repo.entry(PathBuf::from(workdir)).or_default();
        r.push(repo);
    }

    for (workdir, workdir_repos) in workdir_to_repo {
        if workdir_repos.len() > 1 {
            return Err(eyre::eyre!(
                "multiple layer repositories are rooted at git workdir {}",
                workdir.display()
            ));
        }
    }

    Ok(())
}

pub fn ssh_agent_remote_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key_from_agent(username_from_url.unwrap())
    });
    callbacks
}

pub fn local_branch_exists(repo: &Repository, local_branch_name: &str) -> YbResult<bool> {
    match repo.find_branch(local_branch_name, BranchType::Local) {
        Ok(_) => Ok(true),
        Err(err) if err.code() == ErrorCode::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}
