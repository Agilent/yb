use eyre::WrapErr;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use crate::data_model::Layer;
use bytebraise::data_smart::variable_contents::VariableContentsAccessors;
use bytebraise::data_smart::DataSmart;
use bytebraise::parser::parse_bitbake_from_str;
use bytebraise::syntax::ast::evaluate::Evaluate;
use bytebraise::syntax::ast::AstNode;

use crate::errors::YbResult;
use crate::util::paths::normalize_path;

pub struct BBLayersManager {}

impl BBLayersManager {}

pub fn read_bblayers(build_dir: &PathBuf) -> YbResult<HashSet<Layer>> {
    let bblayers = build_dir.join("conf").join("bblayers.conf");

    if bblayers.is_file() {
        let mut source = String::new();
        File::open(&bblayers)
            .with_context(|| format!("failed to read {:?}", &bblayers))?
            .read_to_string(&mut source)?;
        let res = parse_bitbake_from_str(&*source).clone_for_update();
        let d = DataSmart::new();
        res.evaluate(&d).unwrap();
        //TODO .with_context(|| format!("failed to evaluate AST for {:?}", &bblayers))?;

        Ok(d.get_var("BBLAYERS")
            .unwrap()
            .as_string_or_empty()
            .split_whitespace()
            .map(|l| {
                let path = normalize_path(l);
                Layer {
                    path: path.clone(),
                    name: path.file_name().unwrap().to_str().unwrap().to_string(),
                }
            })
            .collect::<HashSet<_>>())
    } else {
        Ok(HashSet::new())
    }
}
