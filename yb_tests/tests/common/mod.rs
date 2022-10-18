use std::path::Path;
pub use yb::util::debug_temp_dir::DebugTempDir;
use assert_cmd::Command;


pub fn yb_cmd<P: AsRef<Path>>(cwd: P) -> Command {
    let mut ret = Command::cargo_bin("yb").unwrap();
    ret.current_dir(cwd).env_clear().env("NO_COLOR", "1");
    if let Ok(var) = std::env::var("CONCURRENT_GIT_POOL") {
        ret.env("CONCURRENT_GIT_POOL", var);
    }
    ret
}
