use async_trait::async_trait;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use bytebraise::editor::list_var_editor::ListVarEditor;

use crate::commands::sync::actions::SyncAction;
use crate::errors::YbResult;
use crate::util::git::pool_helper::PoolHelper;
use crate::util::paths::normalize_path;

#[derive(Debug, PartialEq, Eq)]
pub enum BBLayersEditAction {
    AddLayer,
    RemoveLayer,
}

#[derive(Debug)]
pub struct ModifyBBLayersConfSyncAction {
    layer_path: PathBuf,
    bblayers_path: PathBuf,
    action: BBLayersEditAction,
}

impl ModifyBBLayersConfSyncAction {
    pub fn new(layer_path: PathBuf, bblayers_path: PathBuf, action: BBLayersEditAction) -> Self {
        Self {
            layer_path,
            bblayers_path,
            action,
        }
    }
}

#[async_trait]
impl SyncAction for ModifyBBLayersConfSyncAction {
    fn is_force_required(&self) -> bool {
        false
    }

    async fn apply(&self, pool: &PoolHelper) -> YbResult<()> {
        let layer_path = normalize_path(&self.layer_path)
            .to_str()
            .unwrap()
            .to_string();
        if !self.bblayers_path.is_file() {
            assert_eq!(self.action, BBLayersEditAction::AddLayer);

            fs::create_dir_all(self.bblayers_path.parent().unwrap())?;

            // Generate new bblayers.conf
            let mut bblayers_content = String::from(
                r##"# POKY_BBLAYERS_CONF_VERSION is increased each time build/conf/bblayers.conf
# changes incompatibly
POKY_BBLAYERS_CONF_VERSION = "2"

BBPATH = "${TOPDIR}"
BBFILES ??= """##,
            );

            bblayers_content.push_str(&format!("\n\nBBLAYERS ?= \"{}\"", layer_path));

            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.bblayers_path)?;
            f.write_all(bblayers_content.as_bytes())?;
            return Ok(());
        }

        let mut editor =
            ListVarEditor::from_file(&self.bblayers_path, String::from("BBLAYERS")).unwrap();
        match self.action {
            BBLayersEditAction::AddLayer => {
                editor.add_value(layer_path);
            }
            BBLayersEditAction::RemoveLayer => {
                editor.remove_value(layer_path);
            }
        }
        editor.commit().unwrap();

        Ok(())
    }
}
