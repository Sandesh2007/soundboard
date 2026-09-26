use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundEntry {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
}

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
        self.sounds.push(SoundEntry { id, name, path });
        id
    }

    pub fn remove(&mut self, id: u64) {
        self.sounds.retain(|s| s.id != id);
    }

    pub fn rename(&mut self, id: u64, name: String) {
        if let Some(entry) = self.sounds.iter_mut().find(|s| s.id == id) {
            entry.name = name;
        }
    }
}
