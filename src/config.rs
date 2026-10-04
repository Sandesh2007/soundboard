// This will handle the `config.toml` file that will have app configs.
// I wanted to learn about handling configs so i decided to make seperate
// configs for the actual app config and sounds.

use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    // to use path provided to the device below in
    // `dev_kdb` or just read from `/dev/input/event*`
    use_kbd_path: bool,
    // path for keyboard device
    // eg: `/dev/input/by-id/something_event-kbd`
    //
    dev_kdb: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            use_kbd_path: false,
            dev_kdb: String::new(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path();

        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path)?;

        let config = toml::from_str(&content)?;

        Ok(config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;

        fs::write(path, content)?;

        Ok(())
    }

    fn path() -> PathBuf {
        PathBuf::from("config.toml")
    }
}
