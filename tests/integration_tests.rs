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

#[test]
fn yb_upgrade() -> Result<()> {
    // Test that `yb upgrade` can upgrade an existing Yocto env
    let t = DebugTempDir::new()?;
    let path = t.path();

    let yocto_dir = path.join("yocto");
    fs::create_dir(&yocto_dir)?;

    let sources_dir = yocto_dir.join("sources");
    fs::create_dir(&sources_dir)?;

    Command::new("git")
        .current_dir(&sources_dir)
        .arg("clone")
        .arg("https://github.com/yoctoproject/poky.git")
        .unwrap();
    Command::new("git")
        .current_dir(&sources_dir)
        .arg("clone")
        .arg("https://github.com/openembedded/meta-openembedded.git")
        .unwrap();

    let build_dir = yocto_dir.join("build");
    let conf_dir = build_dir.join("conf");
    let bblayers = conf_dir.join("bblayers.conf");
    fs::create_dir_all(conf_dir).unwrap();
    let mut contents =
        r##"# POKY_BBLAYERS_CONF_VERSION is increased each time build/conf/bblayers.conf
# changes incompatibly
POKY_BBLAYERS_CONF_VERSION = "2"

BBPATH = "${TOPDIR}"
BBFILES ??= ""
BBLAYERS ?= " "##
            .to_string();

    contents += sources_dir.join("poky").to_str().unwrap();
    contents.push(' ');
    contents += sources_dir.join("meta-openembedded").to_str().unwrap();
    contents.push('"');

    fs::write(bblayers, contents).unwrap();

    Command::new("sh")
        .current_dir(&yocto_dir)
        .arg("-c")
        .arg(". sources/poky/oe-init-build-env")
        .unwrap();

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
        .success();

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
