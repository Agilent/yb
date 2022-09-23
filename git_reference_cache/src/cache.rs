use futures::future::Shared;
use futures::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::Mutex;
use crate::error::{Error, Result};

#[derive(Debug)]
pub struct ConcurrentGitCache {
    cache: Mutex<HashMap<String, CacheEntry>>,
    root: TempDir,
}

fn dest_dir_for_remote<S>(remote: S) -> String
where
    S: AsRef<str>,
{
    let remote = remote.as_ref();
    let mut hasher = Sha256::new();
    hasher.update(remote);
    format!("{:x}", hasher.finalize())
}

async fn clone_repo(
    root: PathBuf,
    remote: String,
    dest_dir_name: String,
) -> Result<PathBuf> {
    let status = Command::new("git")
        .current_dir(&root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("clone")
        .arg(&remote)
        .arg(&dest_dir_name)
        .status()
        .await?;

    match status.success() {
        true => Ok(root.join(&dest_dir_name)),
        false => Err(Error::CloneFailed(status)),
    }
}

impl ConcurrentGitCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            root: TempDir::new().unwrap(),
        }
    }

    pub async fn clone_in<C, R>(&self, cwd: C, remote: R)
    where
        C: AsRef<Path>,
        R: AsRef<str>,
    {
        self.git_clone_command(remote)
            .await
            .current_dir(cwd)
            .output()
            .await
            .unwrap();
    }

    pub async fn git_clone_command<R>(&self, remote: R) -> Command
    where
        R: AsRef<str>,
    {
        let cache = self.cache.lock().await;
        let remote = remote.as_ref();

        let mut command = Command::new("git");
        command.arg("clone").arg(remote);

        if let Some(entry) = cache.get(remote) {
            let path = match entry.clone() {
                // Repo is already on-disk
                CacheEntry::Available(p) => p,
                CacheEntry::Cloning(future) => {
                    drop(cache);
                    // Clone is in-flight
                    future.await
                }
            }
            .expect("TODO");

            command
                .arg("--reference")
                .arg(path.to_str().unwrap())
                .arg("--dissociate");
        }

        command
    }

    // Clone the given remote and add it to the cache, if not already present.
    // Returns path to the cached git repo.
    pub async fn get_repo_for_remote<R>(&self, remote: R) -> Result<PathBuf>
    where
        R: Into<String>,
    {
        let remote = remote.into();
        let dest_dir_name = dest_dir_for_remote(&remote);

        let root = self.root.path().to_path_buf();

        let mut cache = self.cache.lock().await;

        match cache.entry(remote.clone()) {
            Entry::Occupied(entry) => {
                return match entry.get().clone() {
                    // Repo is already on-disk
                    CacheEntry::Available(p) => p,
                    CacheEntry::Cloning(future) => {
                        drop(cache);
                        // Clone is in-flight
                        future.await
                    }
                };
            }
            Entry::Vacant(entry) => {
                let request = clone_repo(root, remote.clone(), dest_dir_name)
                    .boxed()
                    .shared();

                entry.insert(CacheEntry::Cloning(request.clone()));

                drop(cache);

                let ret = request.await;

                // Re-acquire lock on HashMap so we can change the entry
                let mut requests = self.cache.lock().await;
                requests.insert(remote.clone(), CacheEntry::Available(ret.clone()));

                ret
            }
        }
    }
}

#[derive(Debug, Clone)]
enum CacheEntry {
    Available(Result<PathBuf>),
    Cloning(Shared<Pin<Box<dyn Future<Output = Result<PathBuf>> + Send>>>),
}

// pub static GIT_CACHE: Lazy<ConcurrentGitCache> = Lazy::new(ConcurrentGitCache::new);
//
// #[cfg(test)]
// mod test {
//     use crate::util::git::concurrent_git_cache::GIT_CACHE;
//     use tokio::join;
//
//     #[tokio::test]
//     async fn clone1() {
//         let poky = GIT_CACHE.get_repo_for_remote("https://github.com/yoctoproject/poky.git");
//
//         let bitbake =
//             GIT_CACHE.get_repo_for_remote("https://github.com/openembedded/meta-openembedded.git");
//
//         join!(poky, bitbake);
//     }
//
//     #[tokio::test]
//     async fn clone2() {
//         let bitbake =
//             GIT_CACHE.get_repo_for_remote("https://github.com/openembedded/meta-openembedded.git");
//         let poky = GIT_CACHE.get_repo_for_remote("https://github.com/yoctoproject/poky.git");
//
//         join!(poky, bitbake);
//     }
// }
