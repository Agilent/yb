use color_eyre::eyre::Result;
use std::fs;
use concurrent_git_pool_proc_macros::clone_repos;
use crate::common::DebugTempDir;
use crate::common::yb_cmd;

mod common;

#[tokio::test]
async fn bare_poky_not_supported() -> Result<()> {
    let t = DebugTempDir::new()?;
    let path = t.path();

    clone_repos! {
        "https://github.com/yoctoproject/poky.git" in &path,
    }

    let poky_dir = path.join("poky");
    let build_dir = poky_dir.join("build");
    fs::create_dir(&build_dir)?;

    let path_var = std::env::var("PATH").unwrap();
    let path_var = format!(
        "{}:{}:{}",
        poky_dir.join("scripts").to_str().unwrap(),
        poky_dir
            .join("bitbake")
            .join("bin")
            .to_str()
            .unwrap(),
        path_var
    );

    yb_cmd(poky_dir)
        .arg("upgrade")
        .env("PATH", path_var)
        .env("BBPATH", build_dir.to_str().unwrap())
        .assert()
        .failure();

    Ok(())
}
