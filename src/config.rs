// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub yt_dlp_cmd: String,
    pub mpv_cmd: String,
    pub curl_cmd: String,
    pub results: usize,
    pub ytdl_format: String,
    pub history_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            yt_dlp_cmd: "yt-dlp".to_string(),
            mpv_cmd: "mpv".to_string(),
            curl_cmd: "curl".to_string(),
            results: 20,
            ytdl_format: "bestvideo[height<=720]+bestaudio/best".to_string(),
            history_size: 100,
        }
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get() -> &'static Config {
    CONFIG.get_or_init(load_or_create)
}

pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("ytglide");
    }
    let home = std::env::var("HOME").expect("HOME is not set");
    PathBuf::from(home).join(".config").join("ytglide")
}

fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

fn load_or_create() -> Config {
    let path = config_file();
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("could not parse {}: {e}, using defaults", path.display());
                Config::default()
            }
        },
        Err(_) => {
            let cfg = Config::default();
            if let Err(e) = write_default(&path, &cfg) {
                eprintln!("could not create {}: {e}", path.display());
            }
            cfg
        }
    }
}

fn write_default(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(cfg).expect("Config is always serializable");
    std::fs::write(path, json)
}
