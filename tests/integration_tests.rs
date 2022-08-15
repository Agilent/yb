use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use color_eyre::eyre::Result;
use lazy_static::lazy_static;
use serde_json::Value;

use crate::common::DebugTempDir;

mod common;

lazy_static! {
    static ref YB_EXE_PATH: PathBuf = get_yb_exe_path().unwrap();
}

fn yb_cmd<P: AsRef<Path>>(cwd: P) -> Command {
    let mut ret = Command::new(&*YB_EXE_PATH);
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

fn get_workspace_root() -> Result<PathBuf> {
    // cargo metadata --format-version=1
    let start_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .current_dir(start_dir)
        .output()?;

    let r: Value = serde_json::from_slice(output.stdout.as_slice())?;
    let workspace_root = r.get("workspace_root").unwrap().as_str().unwrap();
    Ok(PathBuf::from(workspace_root))
}

fn get_yb_exe_path() -> Result<PathBuf> {
    let workspace_root = get_workspace_root()?;
    let target_triple = std::env::var("TARGET").unwrap();
    let exe_path = workspace_root
        .join("target")
        .join(&target_triple)
        .join("debug")
        .join("yb");
    assert!(exe_path.is_file());
    Ok(exe_path)
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
