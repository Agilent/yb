use std::fs;
use std::path::Path;

use assert_cmd::Command;
use color_eyre::eyre::Result;

use crate::common::DebugTempDir;

mod common;

fn yb_cmd<P: AsRef<Path>>(cwd: P) -> Command {
    let mut ret = Command::cargo_bin("yb").unwrap();
    ret.current_dir(cwd).env_clear().env("NO_COLOR", "1");
    ret
}

#[test]
fn yb_init_bare() -> Result<()> {
    let t = DebugTempDir::new()?;
    let path = t.path();
    yb_cmd(path).arg("init").assert().success();
    assert!(path.join("yocto").is_dir());
    assert!(path.join("yocto").join(".yb").is_dir());
    assert!(path.join("yocto").join("sources").is_dir());
    assert!(path.join("yocto").join("build").is_dir());
    Ok(())
}

#[test]
fn no_yb_init_over_existing() -> Result<()> {
    let t = DebugTempDir::new()?;
    let path = t.path();
    // first init should work
    yb_cmd(path).arg("init").assert().success();
    // second init should fail
    yb_cmd(path).arg("init").assert().code(1);
    Ok(())
}

#[test]
fn yb_init() -> Result<()> {
    let conf_repo = create_yb_conf_repo()?;

    let t = DebugTempDir::new()?;
    let path = t.path();

    let yb_env_dir = path.join("yocto");

    yb_cmd(path).arg("init").assert().success();
    yb_cmd(&yb_env_dir)
        .arg("stream")
        .arg("add")
        .arg(conf_repo.path.path())
        .assert()
        .success();
    yb_cmd(&yb_env_dir)
        .arg("activate")
        .arg("zeus")
        .assert()
        .success();
    yb_cmd(&yb_env_dir).arg("sync").arg("-a").assert().success();

    Ok(())
}


fn create_yb_conf_repo() -> Result<GitRepo> {
    let dir = DebugTempDir::new().unwrap();
    let dir_path = dir.path().to_path_buf();

    Command::new("git")
        .arg("init")
        .current_dir(&dir_path)
        .output()?;

    let basic_yaml = include_bytes!("resources/confs/basic.yaml");
    fs::write(dir_path.join("basic.yaml"), basic_yaml).unwrap();

    Command::new("git")
        .current_dir(&dir_path)
        .arg("add")
        .arg("basic.yaml")
        .output()?;

    Command::new("git")
        .current_dir(&dir_path)
        .arg("commit")
        .arg("-m")
        .arg("'initial'")
        .output()?;

    Ok(GitRepo { path: dir })
}

#[derive(Debug)]
struct GitRepo {
    path: DebugTempDir,
}
