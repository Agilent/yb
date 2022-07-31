use ::std::io::Result;
use ::std::path::{Path, PathBuf};

use tempfile::{Builder, TempDir};

// Based on https://gist.github.com/ExpHP/facc0dcbf4399aac7af87dcebae03f7c

#[derive(Debug)]
pub struct DebugTempDir(Option<TempDir>);

impl From<TempDir> for DebugTempDir {
    fn from(tmp: TempDir) -> Self {
        DebugTempDir(Some(tmp))
    }
}

/// Forward everything to the tempdir crate.
impl DebugTempDir {
    pub fn prefixed(prefix: &str) -> Result<DebugTempDir> {
        Builder::new().prefix(prefix).tempdir().map(Self::from)
    }

    pub fn prefixed_in<P: AsRef<Path>>(tmpdir: P, prefix: &str) -> Result<DebugTempDir> {
        Builder::new()
            .prefix(prefix)
            .tempdir_in(tmpdir)
            .map(Self::from)
    }

    pub fn new() -> Result<DebugTempDir> {
        TempDir::new().map(Self::from)
    }

    pub fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
    pub fn into_path(mut self) -> PathBuf {
        self.0.take().unwrap().into_path()
    }
    pub fn close(mut self) -> Result<()> {
        self.0.take().unwrap().close()
    }
}

impl AsRef<Path> for DebugTempDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref().unwrap().as_ref()
    }
}

/// Leaks the inner TempDir if we are unwinding.
impl Drop for DebugTempDir {
    fn drop(&mut self) {
        if ::std::thread::panicking() {
            self.0
                .as_ref()
                .map(|d| eprintln!("retaining temporary directory at: {:?}", d));
            ::std::mem::forget(self.0.take())
        }
    }
}
