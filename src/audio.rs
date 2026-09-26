use anyhow::{Context as _, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub struct AudioEngine {
    // Must be kept alive for as long as sinks should be able to play.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sinks: Vec<Sink>,
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

    pub fn play(&mut self, path: &Path, volume: f32) -> Result<()> {
        // Drop sinks that have already finished so the pool doesn't grow forever.
        self.sinks.retain(|sink| !sink.empty());

        let file = File::open(path)
            .with_context(|| format!("failed to open sound file {}", path.display()))?;
        let source = Decoder::new(BufReader::new(file))
            .with_context(|| format!("failed to decode sound file {}", path.display()))?;

        let sink = Sink::try_new(&self.handle).context("failed to create audio sink")?;
        sink.set_volume(volume);
        sink.append(source);
        self.sinks.push(sink);
        Ok(())
    }

    pub fn stop_all(&mut self) {
        for sink in self.sinks.drain(..) {
            sink.stop();
        }
    }
}
