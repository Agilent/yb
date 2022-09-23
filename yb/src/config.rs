use std::path::PathBuf;

use crate::yb_options::YbOptions;

/// Application-scope context
#[derive(Debug, Clone)]
pub struct Config {
    /// The current working directory
    pub(crate) cwd: PathBuf,
    pub(crate) porcelain: bool,
    pub(crate) git_cache_socket: Option<String>,
}

impl Config {
    pub fn new(cwd: PathBuf, options: &YbOptions) -> Config {
        Config {
            cwd,
            porcelain: options.porcelain,
            git_cache_socket: options.git_cache_socket.clone(),
        }
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    pub fn clone_with_cwd(&self, cwd: PathBuf) -> Config {
        Config {
            cwd,
            ..self.clone()
        }
    }
}
