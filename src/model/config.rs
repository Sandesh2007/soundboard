use crate::model::sound::SoundEntry;

#[derive(Default)]
pub struct Config {
    pub sounds: Vec<SoundEntry>,
    pub next_id: u32,
}
