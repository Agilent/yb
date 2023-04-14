use async_trait::async_trait;
use std::ffi::OsString;
use std::process::Command;

use console::Style;
use eyre::Context;
use indicatif::MultiProgress;

use crate::commands::SubcommandRunner;
use crate::core::tool_context::require_tool_context;
use crate::errors::YbResult;
use crate::util::paths::make_relative_to_cwd;
use crate::Config;

/// Run a command on each top-level layer repository. Works like 'mr run'.
#[derive(Debug, clap::Parser)]
#[clap(setting = clap::AppSettings::TrailingVarArg)]
pub struct RunCommand {
    /// Command and arguments to run on all top-level layer repositories
    #[structopt(parse(from_os_str))]
    args: Vec<OsString>,

    /// Don't print return codes
    #[structopt(name = "no-return-codes", short, long)]
    flag_no_return_codes: bool,
}

#[async_trait]
impl SubcommandRunner for RunCommand {
    async fn run(&self, config: &mut Config, _mp: &MultiProgress) -> YbResult<()> {
        let context = require_tool_context(config)?;
        let repos = context
            .sources_repos()
            .context("can't enumerate layer repos - was the Yocto environment activated?")?;
        if self.args.is_empty() {
            return Err(eyre::eyre!("must pass a command"));
        }

        for repo in &repos {
            let dname_path = repo.workdir().unwrap();
            let dname = dname_path
                .file_name()
                .ok_or_else(|| eyre::eyre!("workdir has no path name?"))?;

            let header = Style::from_dotted_str("blue.bold").apply_to(dname.to_str().unwrap());

            println!(
                "\n{} [{}]:",
                header,
                make_relative_to_cwd(dname_path).unwrap().display()
            );

            let result = Command::new(&self.args[0])
                .args(&self.args[1..])
                .current_dir(repo.workdir().unwrap())
                .spawn()?
                .wait()?;

            if !self.flag_no_return_codes {
                let (color, return_code_text) = match result.code() {
                    Some(0) => (Style::from_dotted_str("green"), String::from("0")),
                    Some(code) => (Style::from_dotted_str("red"), code.to_string()),
                    None => (
                        Style::from_dotted_str("yellow"),
                        String::from("[terminated by signal]"),
                    ),
                };

                println!(
                    "{}: {}",
                    color.bold().apply_to("return code"),
                    return_code_text
                );
            }
        }

        Ok(())
    }
}
