use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub last_synced_at: DateTime<Utc>,
    pub last_nsr: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_synced_at: DateTime::<Utc>::MIN_UTC,
            last_nsr: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: String,
    pub status: String,
    pub records_sent: u32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logs {
    pub entries: Vec<LogEntry>,
    pub next_id: u64,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
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

pub fn save_state(state: &State) -> Result<(), io::Error> {
    let config_dir = get_config_dir()?;
    let state_path = config_dir.join("state.json");

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(state_path, json)?;

    log::debug!(
        "State saved: last_nsr={}, last_synced={}",
        state.last_nsr,
        state.last_synced_at
    );
    Ok(())
}

pub fn load_state() -> Result<State, io::Error> {
    let config_dir = get_config_dir()?;
    let state_path = config_dir.join("state.json");

    if !state_path.exists() {
        log::info!("State file not found, using default state");
        return Ok(State::default());
    }

    let json = fs::read_to_string(state_path)?;
    let state: State =
        serde_json::from_str(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(state)
}

pub fn save_log(status: &str, records_sent: u32, message: &str) -> Result<(), io::Error> {
    let mut logs = load_logs()?;

    let entry = LogEntry {
        id: logs.next_id,
        timestamp: Utc::now().format("%d/%m/%Y %H:%M:%S").to_string(),
        status: status.to_string(),
        records_sent,
        message: message.to_string(),
    };

    logs.entries.insert(0, entry);
    logs.next_id += 1;

    if logs.entries.len() > 100 {
        logs.entries.truncate(100);
    }

    save_logs(&logs)
}

pub fn save_logs(logs: &Logs) -> Result<(), io::Error> {
    let config_dir = get_config_dir()?;
    let logs_path = config_dir.join("logs.json");

    let json = serde_json::to_string_pretty(logs)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(logs_path, json)?;

    Ok(())
}

pub fn load_logs() -> Result<Logs, io::Error> {
    let config_dir = get_config_dir()?;
    let logs_path = config_dir.join("logs.json");

    if !logs_path.exists() {
        return Ok(Logs::default());
    }

    let json = fs::read_to_string(logs_path)?;
    let logs: Logs =
        serde_json::from_str(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(logs)
}
