use crate::util::debug_temp_dir::DebugTempDir;
use assert_cmd::Command;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

enum CacheEntry {
    Cloning(Arc<(Mutex<()>, Condvar)>),
    Cloned(PathBuf),
}

pub struct ConcurrentGitCache {
    root: DebugTempDir,
    cloning: Mutex<HashMap<String, CacheEntry>>,
}

impl ConcurrentGitCache {
    pub fn new() -> Self {
        Self {
            root: DebugTempDir::new().unwrap(),
            cloning: Mutex::new(HashMap::new()),
        }
    }

    pub fn lookup_or_clone<S>(&self, remote: S)
    where
        S: AsRef<str>,
    {
        let remote = remote.as_ref();
        let mut state = self.cloning.lock().unwrap();

        match state.entry(remote.to_string()) {
            Occupied(a) => {
                if let CacheEntry::Cloning(arc) = a.get() {
                    let arc = arc.clone();
                    drop(state);

                    let lock = arc.0.lock().unwrap();
                    let _ = arc.1.wait(lock).unwrap();

                    eprintln!("waited!");
                    let state = self.cloning.lock().unwrap();
                    let p = state.get(remote).unwrap();
                    if let CacheEntry::Cloned(p) = p {
                        panic!("{:?}", p);
                    }
                }
            }
            Vacant(a) => {
                let mutex = Mutex::new(());
                let cv = Condvar::new();
                let arc = Arc::new((mutex, cv));
                let entry = CacheEntry::Cloning(arc.clone());
                a.insert(entry);

                drop(state);

                Command::new("git")
                    .current_dir(&self.root)
                    .arg("clone")
                    .arg(remote)
                    .unwrap();

                let mut state = self.cloning.lock().unwrap();

                arc.1.notify_all();

                if let Occupied(mut a) = state.entry(remote.to_string()) {
                    a.insert(CacheEntry::Cloned(PathBuf::new()));
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::util::git::concurrent_git_cache::ConcurrentGitCache;
    use once_cell::sync::{Lazy};

    static INSTANCE: Lazy<ConcurrentGitCache> = Lazy::new(|| ConcurrentGitCache::new());

    #[tokio::test]
    async fn clone1() {
        INSTANCE.lookup_or_clone("https://github.com/yoctoproject/poky.git");
    }

    #[tokio::test]
    async fn clone2() {
        INSTANCE.lookup_or_clone("https://github.com/yoctoproject/poky.git");
    }
}
