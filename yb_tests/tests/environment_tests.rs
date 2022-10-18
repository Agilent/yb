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

    let yocto_dir = path.join("yocto");
    fs::create_dir(&yocto_dir)?;

    let sources_dir = yocto_dir.join("sources");
    fs::create_dir(&sources_dir)?;

    clone_repos! {
        "https://github.com/yoctoproject/poky.git" in &sources_dir,
    }

    let build_dir = yocto_dir.join("build");
    fs::create_dir(&build_dir)?;

    let path_var = std::env::var("PATH").unwrap();
    let path_var = format!(
        "{}:{}:{}",
        sources_dir.join("poky").join("scripts").to_str().unwrap(),
        sources_dir
            .join("poky")
            .join("bitbake")
            .join("bin")
            .to_str()
            .unwrap(),
        path_var
    );

    yb_cmd(yocto_dir)
        .arg("upgrade")
        .env("PATH", path_var)
        .env("BBPATH", build_dir.to_str().unwrap())
        .assert()
        .failure();
    
    Ok(())
}
