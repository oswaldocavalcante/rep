use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

/// URL fixa do sistema Ryanne — não configurável pelo usuário.
pub const RYANNE_API_URL: &str = "https://sistema.ryanne.com.br";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub device_ip: String,
    pub device_user: String,
    pub device_password: String,
    pub api_key: String,
    pub clock_id: String,
    pub sync_interval_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_ip: String::new(),
            device_user: "admin".to_string(),
            device_password: String::new(),
            api_key: String::new(),
            clock_id: String::new(),
            sync_interval_secs: 300,
        }
    }
}

fn get_config_dir() -> Result<PathBuf, io::Error> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Config directory not found"))?
        .join("ryanne-ponto");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    Ok(config_dir)
}

pub fn save_config(config: &Config) -> Result<(), io::Error> {
    let config_dir = get_config_dir()?;
    let config_path = config_dir.join("config.toml");

    let toml = toml::to_string_pretty(config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(config_path, toml)?;

    log::info!("Config saved successfully");
    Ok(())
}

pub fn load_config() -> Result<Config, io::Error> {
    let config_dir = get_config_dir()?;
    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        log::info!("Config file not found, using default config");
        return Ok(Config::default());
    }

    let toml = fs::read_to_string(config_path)?;
    let config: Config =
        toml::from_str(&toml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(config)
}
