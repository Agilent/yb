use crate::commands::Subcommands;
use crate::VERSION;

#[derive(clap::Parser, Debug)]
#[clap(name = "yb", about = "Yocto buddy", version = VERSION)]
pub struct YbOptions {
    /// Set log level
    #[clap(short = 'v', long, global = true, value_enum, default_value = "warn")]
    pub level: Level,

    /// Coloring: auto, always, never
    #[clap(long, global = true)]
    pub color: Option<String>,

    #[clap(long, global = true)]
    pub porcelain: bool,

    #[clap(subcommand)]
    pub command: Subcommands,

    #[clap(long, global = true)]
    pub git_cache_socket: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<Level> for tracing::Level {
    fn from(level: Level) -> Self {
        match level {
            Level::Error => tracing::Level::ERROR,
            Level::Warn => tracing::Level::WARN,
            Level::Info => tracing::Level::INFO,
            Level::Debug => tracing::Level::DEBUG,
            Level::Trace => tracing::Level::TRACE,
        }
    }
}
