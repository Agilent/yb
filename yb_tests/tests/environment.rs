use color_eyre::eyre::Result;
use std::fs;
use concurrent_git_pool::PoolHelper;
use yb::util::debug_temp_dir::DebugTempDir;

#[tokio::test]
async fn bare_poky_not_supported() -> Result<()> {
    let t = DebugTempDir::new()?;
    let path = t.path();

    let yocto_dir = path.join("yocto");
    fs::create_dir(&yocto_dir)?;

    let sources_dir = yocto_dir.join("sources");
    fs::create_dir(&sources_dir)?;

   

    Ok(())
}
