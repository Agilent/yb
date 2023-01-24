use crate::errors::YbResult;
use crate::spec::{ActiveSpec, Spec};
use crate::stream::Stream;
use crate::util::paths::is_hidden;
use eyre::Context;
use slotmap::{new_key_type, SlotMap};
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

new_key_type! {
    pub struct StreamKey;
}

#[derive(Debug)]
pub struct StreamDb {
    streams: SlotMap<StreamKey, Stream>,
}

impl StreamDb {
    pub fn new() -> Self {
        Self {
            streams: SlotMap::with_key(),
        }
    }

    pub fn streams(&self) -> slotmap::basic::Iter<'_, StreamKey, Stream> {
        self.streams.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    pub fn has_broken(&self) -> bool {
        self.streams.iter().any(|stream| stream.1.is_broken())
    }

    pub fn broken_streams(&self) -> HashMap<StreamKey, Arc<eyre::Report>> {
        self.streams
            .iter()
            .filter_map(|stream| {
                stream.1.broken_reason().map(|reason| (stream.0, reason))
            })
            .collect()
    }

    pub fn load_all<P: AsRef<Path>>(&mut self, streams_dir: P) -> YbResult<()> {
        // Iterate over each stream (which are subdirectories)
        for d in WalkDir::new(streams_dir)
            .max_depth(1)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .filter(|e| e.as_ref().unwrap().file_type().is_dir())
        {
            let stream_path = d?.into_path();
            let stream_name = stream_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            self.streams.insert_with_key(|key| {
                Stream::load(stream_path.clone(), stream_name, key).unwrap()
            });
        }

        Ok(())
    }

    pub fn get_stream_by_name<N: AsRef<str>>(&self, name: N) -> Option<&Stream> {
        let name = name.as_ref();
        self.streams
            .iter()
            .find(|stream| stream.1.name() == name)
            .map(|item| item.1)
    }

    pub fn find_spec_by_name<N: AsRef<str>>(&self, name: N) -> YbResult<Option<&Spec>> {
        let mut ret = None;

        for stream in self.streams.values() {
            let s = stream.get_spec_by_name(&name);
            if s.is_some() {
                if ret.is_some() {
                    eyre::bail!("spec '{}' found in multiple streams", name.as_ref());
                }
                ret = s;
            }
        }

        Ok(ret)
    }

    pub fn stream(&self, stream_key: StreamKey) -> Option<&Stream> {
        self.streams.get(stream_key)
    }

    pub fn stream_mut(&mut self, stream_key: StreamKey) -> Option<&mut Stream> {
        self.streams.get_mut(stream_key)
    }

    pub fn load_active_spec(&self, active_spec_file_path: PathBuf) -> YbResult<ActiveSpec> {
        let active_spec_file = File::open(&active_spec_file_path)?;
        let mut active_spec = serde_yaml::from_reader::<_, ActiveSpec>(active_spec_file)
            .with_context(|| {
                format!(
                    "failed to parse active spec file {}",
                    &active_spec_file_path.display()
                )
            })?;

        if let Some(stream) = self.get_stream_by_name(&active_spec.from_stream) {
            if stream.get_spec_by_name(active_spec.name()).is_none() {
                eyre::bail!("active spec '{}' claims to be a member of stream '{}', but it was not found there", active_spec.name(), active_spec.from_stream);
            }

            active_spec.stream_key = stream.key();
        } else {
            eyre::bail!(
                "active spec '{}' refers to non-existent stream '{}'",
                active_spec.name(),
                active_spec.from_stream
            );
        }

        Ok(active_spec)
    }

    pub fn make_active_spec(&self, spec: Spec) -> YbResult<ActiveSpec> {
        if let Some(stream) = self.streams.get(spec.stream_key) {
            let key = spec.stream_key;
            Ok(ActiveSpec {
                spec,
                from_stream: stream.name().clone(),
                stream_key: key,
            })
        } else {
            eyre::bail!("spec '{}' not found in any stream", spec.name())
        }
    }
}
