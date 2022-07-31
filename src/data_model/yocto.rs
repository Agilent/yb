use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize)]
pub struct Layer {
    pub path: PathBuf,
    pub name: String,
}
