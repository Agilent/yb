use eyre::{Context, ContextCompat};
use std::os::linux::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::{env, fs, io};

use walkdir::DirEntry;

use crate::errors::YbResult;

pub fn run_which(program_name: &str) -> YbResult<Option<PathBuf>> {
    let output = Command::new("which").arg(program_name).output()?;
    return Ok(match output.status.code() {
        Some(0) => Some(PathBuf::from(String::from_utf8(output.stdout)?.trim_end())),
        _ => None,
    });
}

pub fn make_relative_to_cwd<P: ?Sized>(path: &P) -> YbResult<PathBuf>
where
    P: AsRef<Path>,
{
    try_diff_paths(path, env::current_dir()?)
}

pub fn try_diff_paths<P, B>(path: P, base: B) -> YbResult<PathBuf>
where
    P: AsRef<Path>,
    B: AsRef<Path>,
{
    pathdiff::diff_paths(path, base).context("unable to compute relative path")
}

pub fn list_subdirectories_sorted(dir: &Path) -> YbResult<Vec<PathBuf>> {
    let mut ret: Vec<PathBuf> = fs::read_dir(dir)?
        .into_iter()
        .filter_map(|r| r.ok().map(|r| r.path()))
        .filter(|r| r.is_dir())
        .collect();
    ret.sort();
    Ok(ret)
}

pub fn is_yaml_file(entry: &DirEntry) -> bool {
    entry.file_type().is_file()
        && entry
            .file_name()
            .to_str()
            .map(|s| s.ends_with(".yaml"))
            .unwrap_or(false)
}

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

// from https://stackoverflow.com/a/68233480
/// Improve the path to try remove and solve .. token.
///
/// This assumes that `a/b/../c` is `a/c` which might be different from
/// what the OS would have chosen when b is a link. This is OK
/// for broot verb arguments but can't be generally used elsewhere
///
/// This function ensures a given path ending with '/' still
/// ends with '/' after normalization.
pub fn normalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let ends_with_slash = path.as_ref().to_str().map_or(false, |s| s.ends_with('/'));
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match &component {
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component);
                }
            }
            _ => {
                normalized.push(component);
            }
        }
    }
    if ends_with_slash {
        normalized.push("");
    }
    normalized
}

pub fn find_dir_recurse_upwards<S: AsRef<Path>>(
    start_point: S,
    dir_name: &str,
) -> YbResult<Option<PathBuf>> {
    let start_point = start_point.as_ref();
    assert!(start_point.is_dir());
    let st_dev = start_point.metadata().unwrap().st_dev();

    let mut dir = Some(start_point);
    while let Some(root) = &dir {
        let metadata = root
            .metadata()
            .with_context(|| format!("couldn't get fs metadata for {:?}", &dir))?;

        // Don't cross filesystems
        if metadata.st_dev() != st_dev {
            return Ok(None);
        }

        let candidate = root.join(dir_name);
        match candidate.metadata() {
            Ok(yb_dir_metadata) if yb_dir_metadata.is_dir() => return Ok(Some(candidate)),
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                Err(e).with_context(|| format!("couldn't get fs metadata for {:?}", &candidate))?
            }
            _ => {
                dir = root.parent();
            }
        }
    }

    Ok(None)
}
