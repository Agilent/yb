use std::{env, io};

use clap::Parser;
use eyre::Context;
use indicatif::MultiProgress;

use yb::commands::*;
use yb::config::Config;
use yb::errors::YbResult;
use yb::yb_options::{Level, YbOptions};

fn parse_args_and_create_config() -> YbResult<(Config, YbOptions)> {
    let opt: YbOptions = YbOptions::parse();
    let cwd = env::current_dir().context("couldn't get the current directory of the process")?;
    let config = Config::new(cwd, &opt);
    Ok((config, opt))
}

fn main() {
    let _ = coredump::register_panic_handler();
    if env::var("NO_COLOR") == Err(std::env::VarError::NotPresent) {
        color_eyre::install().unwrap();
    } else {
        color_eyre::config::HookBuilder::new()
            .theme(color_eyre::config::Theme::new())
            .install()
            .unwrap();
    }

    if let Err(code) = real_main() {
        std::process::exit(code);
    }
}

fn real_main() -> Result<(), i32> {
    // Automatically enable backtracing unless user explicitly disabled it
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "1");
    }

    // Figure out what we're going to do
    let result = parse_args_and_create_config();
    match result {
        Err(err) => {
            eprintln!("internal error whilst setting up application: {:?}", err);
            return Err(1);
        }

        Ok((mut config, opt)) => {
            let mp = MultiProgress::new();

            install_tracing(opt.level, mp.clone());

            // Run the subcommand
            if let Err(err) = opt.command.run(&mut config, &mp) {
                eprintln!("internal error: {:?}", err);
                return Err(1);
            }
        }
    }

    Ok(())
}

fn install_tracing(level: Level, mp: MultiProgress) {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::fmt;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_writer(move || MultiProgressWriteWrapper::new(mp.clone()));
    let level = tracing::Level::from(level);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| {
            EnvFilter::builder()
                .with_default_directive(level.into())
                .parse("")
        })
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

struct MultiProgressWriteWrapper(MultiProgress);

impl MultiProgressWriteWrapper {
    fn new(mp: MultiProgress) -> Self {
        Self(mp)
    }
}

impl io::Write for MultiProgressWriteWrapper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.suspend(|| io::stderr().lock().write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
