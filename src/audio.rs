use anyhow::{Context as _, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

/// Owns the default audio output and a pool of active playback sinks, each
/// tagged with the id of the sound it's playing so individual sounds can be
/// paused/resumed independently rather than only stopped as a group.
pub struct AudioEngine {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sinks: Vec<(String, Sink)>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("failed to open default audio output")?;
        Ok(Self {
            _stream: stream,
            handle,
            sinks: Vec::new(),
        })
    }

    pub fn play(&mut self, id: String, path: &Path, volume: f32) -> Result<Option<Duration>> {
        self.sinks.retain(|(_, sink)| !sink.empty());
        if let Some(pos) = self.sinks.iter().position(|(sink_id, _)| sink_id == &id) {
            let (_, old_sink) = self.sinks.remove(pos);
            old_sink.stop();
        }

        let file = File::open(path)
            .with_context(|| format!("failed to open sound file {}", path.display()))?;
        let source = Decoder::new(BufReader::new(file))
            .with_context(|| format!("failed to decode sound file {}", path.display()))?;

        let duration = source.total_duration();

        let sink = Sink::try_new(&self.handle).context("failed to create audio sink")?;
        sink.set_volume(volume);
        sink.append(source);
        self.sinks.push((id, sink));
        Ok(duration)
    }

    pub fn pause(&mut self, id: &str) {
        if let Some((_, sink)) = self
            .sinks
            .iter()
            .find(|(sink_id, _)| sink_id.as_str() == id)
        {
            sink.pause();
        }
    }

    pub fn resume(&mut self, id: &str) {
        if let Some((_, sink)) = self
            .sinks
            .iter()
            .find(|(sink_id, _)| sink_id.as_str() == id)
        {
            sink.play();
        }
    }

    pub fn has_finished(&self, id: &str) -> bool {
        !self
            .sinks
            .iter()
            .any(|(sink_id, sink)| sink_id.as_str() == id && !sink.empty())
    }

    pub fn stop_all(&mut self) {
        for (_, sink) in self.sinks.drain(..) {
            sink.stop();
        }
    }
}
