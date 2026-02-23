use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::models::{AppConfig, Preset};

pub struct StorageState {
    pub config: Mutex<AppConfig>,
    pub presets: Mutex<Vec<Preset>>,
    data_dir: PathBuf,
}

impl StorageState {
    pub fn new(data_dir: PathBuf) -> Self {
        fs::create_dir_all(&data_dir).ok();

        let config = Self::load_json::<AppConfig>(&data_dir.join("config.json"))
            .unwrap_or_default();
        let presets = Self::load_json::<Vec<Preset>>(&data_dir.join("presets.json"))
            .unwrap_or_default();

        Self {
            config: Mutex::new(config),
            presets: Mutex::new(presets),
            data_dir,
        }
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<(), String> {
        Self::save_json(&self.data_dir.join("config.json"), config)
    }

    pub fn save_presets(&self, presets: &[Preset]) -> Result<(), String> {
        Self::save_json(&self.data_dir.join("presets.json"), presets)
    }

    fn load_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Option<T> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_json<T: serde::Serialize + ?Sized>(path: &PathBuf, data: &T) -> Result<(), String> {
        let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }
}
