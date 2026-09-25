use crate::model::sound::SoundEntry;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::collections::HashMap;
use std::io::BufReader;

pub struct AudioPlayer {
    // Must stay alive or playback silently dies
    _stream: MixerDeviceSink,
    active: HashMap<u32, Player>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let stream = DeviceSinkBuilder::open_default_sink().expect("no audio output device found");

        Self {
            _stream: stream,
            active: HashMap::new(),
        }
    }

    pub fn play(&mut self, sound: &SoundEntry) {
        if let Ok(file) = std::fs::File::open(&sound.file_path) {
            if let Ok(source) = Decoder::new(BufReader::new(file)) {
                let player = Player::connect_new(self._stream.mixer());

                player.set_volume(sound.volume);
                player.append(source);

                self.active.insert(sound.id, player);
            }
        }
    }

    pub fn stop(&mut self, id: u32) {
        if let Some(player) = self.active.remove(&id) {
            player.stop();
        }
    }

    pub fn stop_all(&mut self) {
        for (_, player) in self.active.drain() {
            player.stop();
        }
    }
}
