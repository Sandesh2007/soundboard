use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SoundEntry {
    pub id: u32,
    pub name: String,
    pub file_path: PathBuf,
    pub volume: f32,
    pub keybind: Option<()>, // placeholder until Phase 6/7
}
