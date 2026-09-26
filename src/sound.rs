use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A single local sound file added to the board.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundEntry {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,

    /// Per-sound volume, linear gain (0.0..=1.0, occasionally boosted above 1.0).
    #[serde(default = "default_volume")]
    pub volume: f32,

    /// Optional keybind, e.g. "ctrl-shift-1". Formatted from GPUI's `Keystroke`.
    #[serde(default)]
    pub keybind: Option<String>,

    /// Optional profile picture / icon shown on the sound's tile.
    #[serde(default)]
    pub image_path: Option<PathBuf>,
}

fn default_volume() -> f32 {
    1.0
}

/// The user's saved set of sounds. Persisted as JSON so sounds survive
/// restarting the app.
#[derive(Default, Serialize, Deserialize)]
pub struct SoundLibrary {
    pub sounds: Vec<SoundEntry>,
    next_id: u64,
}

impl SoundLibrary {
    fn config_path() -> PathBuf {
        let dir = dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("soundboard");
        let _ = fs::create_dir_all(&dir);
        dir.join("sounds.json")
    }

    /// Load the saved library, or an empty one if none exists yet.
    pub fn load() -> Self {
        let path = Self::config_path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Add a sound and return the id it was assigned.
    pub fn add(&mut self, name: String, path: PathBuf) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.sounds.push(SoundEntry {
            id,
            name,
            path,
            volume: default_volume(),
            keybind: None,
            image_path: None,
        });
        id
    }

    pub fn remove(&mut self, id: u64) {
        self.sounds.retain(|s| s.id != id);
    }

    pub fn get(&self, id: u64) -> Option<&SoundEntry> {
        self.sounds.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut SoundEntry> {
        self.sounds.iter_mut().find(|s| s.id == id)
    }

    /// Find a sound whose keybind matches the given combo string, if any.
    pub fn find_by_keybind(&self, combo: &str) -> Option<u64> {
        self.sounds
            .iter()
            .find(|s| s.keybind.as_deref() == Some(combo))
            .map(|s| s.id)
    }
}
