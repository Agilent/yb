use std::path::PathBuf;

use crate::yb_options::YbOptions;

/// Application-scope context
#[derive(Debug, Clone)]
pub struct Config {
    /// The current working directory
    pub(crate) cwd: PathBuf,
    pub(crate) porcelain: bool,
}

impl Config {
    pub fn new(cwd: PathBuf, options: &YbOptions) -> Config {
        Config {
            cwd,
            porcelain: options.porcelain,
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
