use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub log_path: String,
    pub port: u16,
    pub dev_mode: bool,
    pub dev_origins: Vec<String>,
    pub auto_start_watcher: bool,
    #[serde(default)]
    pub custom_origins: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            log_path: default_log_path(),
            port: 23456,
            dev_mode: false,
            dev_origins: vec![],
            auto_start_watcher: true,
            custom_origins: vec![],
        }
    }
}

fn default_log_path() -> String {
    if cfg!(target_os = "windows") {
        r"C:\Program Files\Roberts Space Industries\StarCitizen\LIVE\Game.log".to_string()
    } else {
        String::new()
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("scmdb-watcher");
        std::fs::create_dir_all(&dir).ok();
        dir.join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }

    pub fn allowed_origins(&self) -> Vec<String> {
        let mut origins = vec![
            "https://scmdb.net".to_string(),
            "https://www.scmdb.net".to_string(),
        ];
        for o in &self.custom_origins {
            if !origins.contains(o) {
                origins.push(o.clone());
            }
        }
        if cfg!(debug_assertions) && self.dev_mode {
            origins.push("http://localhost:5173".to_string());
            origins.push("http://localhost:3000".to_string());
            for o in &self.dev_origins {
                if !origins.contains(o) {
                    origins.push(o.clone());
                }
            }
        }
        origins
    }
}
