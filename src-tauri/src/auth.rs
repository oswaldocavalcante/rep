use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Sessão de auth válida por 8 horas
const TOKEN_TTL: Duration = Duration::from_secs(8 * 3600);

// ─── Configuração de credenciais ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub password_hash: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        // Senha padrão: "admin" — deve ser trocada no primeiro uso
        Self {
            password_hash: hash_password("admin"),
        }
    }
}

pub fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

fn get_auth_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/etc/rep"))
        .join("ryanne-ponto")
        .join("auth.toml")
}

pub fn load_auth_config() -> AuthConfig {
    let path = get_auth_config_path();
    if !path.exists() {
        let default = AuthConfig::default();
        let _ = save_auth_config(&default);
        return default;
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    toml::from_str(&content).unwrap_or_default()
}

pub fn save_auth_config(config: &AuthConfig) -> Result<(), io::Error> {
    let path = get_auth_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, content)
}

// ─── Token store (em memória) ─────────────────────────────────────────────────

#[derive(Clone)]
pub struct TokenStore {
    tokens: Arc<Mutex<HashMap<String, Instant>>>,
}

impl TokenStore {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_token(&self) -> String {
        let token = Uuid::new_v4().to_string();
        let mut store = self.tokens.lock().unwrap();
        store.insert(token.clone(), Instant::now());
        token
    }

    pub fn validate_token(&self, token: &str) -> bool {
        let mut store = self.tokens.lock().unwrap();
        match store.get(token) {
            Some(created_at) if created_at.elapsed() < TOKEN_TTL => true,
            Some(_) => {
                store.remove(token);
                false
            }
            None => false,
        }
    }

    pub fn revoke_token(&self, token: &str) {
        let mut store = self.tokens.lock().unwrap();
        store.remove(token);
    }

    pub fn cleanup_expired(&self) {
        let mut store = self.tokens.lock().unwrap();
        store.retain(|_, created_at| created_at.elapsed() < TOKEN_TTL);
    }
}
