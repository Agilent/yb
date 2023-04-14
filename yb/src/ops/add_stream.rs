use assert_cmd::Command;
use eyre::WrapErr;
use std::fs;
use std::fs::OpenOptions;
use std::path::PathBuf;

use git2::build::RepoBuilder;
use git2::FetchOptions;
use tempfile::Builder;

use crate::config::Config;
use crate::core::tool_context::require_yb_env;
use crate::errors::YbResult;
use crate::stream::{
    Stream, StreamConfig, StreamKind, STREAM_CONFIG_FILE, STREAM_CONTENT_ROOT_SUBDIR,
};
use crate::stream_db::StreamKey;
use crate::util::git::ssh_agent_remote_callbacks;

pub struct AddStreamOptions<'cfg> {
    config: &'cfg Config,
    pub(crate) uri: String,
    pub(crate) name: Option<String>,
}

impl<'cfg> AddStreamOptions<'cfg> {
    pub fn new(config: &'cfg Config) -> Self {
        Self {
            config,
            uri: String::new(),
            name: None,
        }
    }

    pub fn uri(&mut self, uri: String) -> &mut AddStreamOptions<'cfg> {
        self.uri = uri;
        self
    }

    pub fn name(&mut self, name: Option<String>) -> &mut AddStreamOptions<'cfg> {
        self.name = name;
        self
    }

    // pub fn callbacks(
    //     &mut self,
    //     callbacks: AddStreamCallbacks<'cfg>,
    // ) -> &mut AddStreamOptions<'cfg> {
    //     self.callbacks = callbacks;
    //     self
    // }
}

pub fn op_add_stream(options: AddStreamOptions) -> YbResult<()> {
    let yb_env = require_yb_env(options.config)?;

    let stream_name = options.name.clone().unwrap_or_else(|| "default".into());

    let tmpdir = Builder::new().prefix("yb").tempdir()?;
    let tmp_contents_dir = tmpdir.path().join(STREAM_CONTENT_ROOT_SUBDIR);

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(ssh_agent_remote_callbacks());

    // Clone the stream
    RepoBuilder::new()
        .fetch_options(fetch_options)
        .clone(&options.uri, tmp_contents_dir.as_ref())?;

    // Write the config file
    // TODO: when other stream types are added, don't hardcode git
    let config = StreamConfig::new(StreamKind::Git);
    let config_file_path = tmpdir.path().join(STREAM_CONFIG_FILE);
    let f = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&config_file_path)
        .context(format!(
            "failed to open file {:?} for writing",
            &config_file_path
        ))?;
    serde_yaml::to_writer(f, &config)?;

    let stream_dir = yb_env.streams_dir();
    if !stream_dir.is_dir() {
        println!("creating dir: {:?}", &stream_dir);
        fs::create_dir(&stream_dir)?;
    }

    let stream_root_dir = stream_dir.join(&stream_name);
    if stream_root_dir.exists() {
        eyre::bail!("a stream with name {} already exists", &stream_name);
    }

    // Just fake a key for now
    let key = StreamKey::default();
    // Try to load stream
    Stream::load(PathBuf::from(tmpdir.path()), stream_name, key)?;

    // Everything was OK, so move into stream directory
    let mut mv_cmd = Command::new("mv");
    mv_cmd.arg(tmpdir.into_path()).arg(&stream_root_dir);
    mv_cmd.assert().success();

    println!("yb {:?}", &yb_env);

    Ok(())
}
