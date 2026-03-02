use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PunchRecord {
    pub employee_code: String,
    pub timestamp: String,
    pub record_type: RecordType,
    /// NSR (Número Sequencial de Registro) original da linha AFD
    pub nsr: u64,
    /// Linha AFD original para rastreabilidade (rawPayload)
    pub raw_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecordType {
    ClockIn,
    ClockOut,
    Unknown,
}

#[derive(Debug)]
pub struct IdClassClient {
    client: Client,
    ip: String,
    session: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordsBatch {
    pub records: Vec<PunchRecord>,
    pub latest_nsr: u64,
}

impl IdClassClient {
    fn normalize_code(value: &str) -> String {
        let digits_only: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits_only.is_empty() {
            return value.trim().to_string();
        }
        let trimmed = digits_only.trim_start_matches('0');
        if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
    }

    pub fn new(ip: &str) -> Self {
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            ip: ip.to_string(),
            session: None,
        }
    }

    pub async fn login(&mut self, user: &str, password: &str) -> Result<String, String> {
        let url = format!("https://{}/login.fcgi", self.ip);
        
        let body = serde_json::json!({
            "login": user,
            "password": password
        });

        let response = self.client
            .post(&url)
            .header("Accept-Encoding", "identity")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Connection error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Login failed: {}", response.status()));
        }

        let json: serde_json::Value = response.json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let session = json["session"]
            .as_str()
            .ok_or("No session in response")?
            .to_string();

        self.session = Some(session.clone());
        let preview_len = session.len().min(8);
        log::info!("Logged in to IDClass, session: {}", &session[..preview_len]);
        
        Ok(session)
    }

    pub async fn get_system_info(&self) -> Result<SystemInfo, String> {
        let session = self.session.as_ref().ok_or("Not logged in")?;
        let url = format!("https://{}/get_system_information.fcgi?session={}", self.ip, session);
        
        let response = self.client
            .post(&url)
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| format!("Connection error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Request failed: {}", response.status()));
        }

        let info: SystemInfo = response.json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(info)
    }

    pub async fn get_afd(&self, initial_nsr: u64) -> Result<String, String> {
        let session = self.session.as_ref().ok_or("Not logged in")?;
        let url = format!("https://{}/get_afd.fcgi?session={}", self.ip, session);
        
        let body = serde_json::json!({
            "initial_nsr": initial_nsr
        });

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Connection error: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Request failed: {}", response.status()));
        }

        let bytes = response.bytes()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let text = match String::from_utf8(bytes.to_vec()) {
            Ok(value) => value,
            Err(_) => String::from_utf8_lossy(&bytes).to_string(),
        };

        Ok(text)
    }

    pub async fn load_user_pis_map(&self) -> Result<HashMap<String, String>, String> {
        let session = self.session.as_ref().ok_or("Not logged in")?;
        let url = format!("https://{}/load_users.fcgi?session={}", self.ip, session);

        let mut map: HashMap<String, String> = HashMap::new();
        let mut pis_values: Vec<String> = Vec::new();
        let mut offset = 0u64;
        let limit = 100u64;
        let mut total_count: Option<u64> = None;

        loop {
            let body = serde_json::json!({
                "offset": offset,
                "limit": limit
            });

            let response = self.client
                .post(&url)
                .header("Accept-Encoding", "identity")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Connection error: {}", e))?;

            if !response.status().is_success() {
                return Err(format!("Request failed: {}", response.status()));
            }

            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse users response: {}", e))?;

            if total_count.is_none() {
                total_count = json.get("count").and_then(|value| value.as_u64());
            }

            let users = json
                .get("users")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();

            if users.is_empty() {
                break;
            }

            for user in &users {
                let value_to_string = |key: &str| -> Option<String> {
                    user.get(key).and_then(|value| {
                        value
                            .as_str()
                            .map(|s| s.to_string())
                            .or_else(|| value.as_i64().map(|n| n.to_string()))
                            .or_else(|| value.as_u64().map(|n| n.to_string()))
                    })
                };

                let Some(pis_raw) = value_to_string("pis") else {
                    continue;
                };
                let pis = Self::normalize_code(&pis_raw);
                if pis.is_empty() || pis == "0" {
                    continue;
                }

                pis_values.push(pis.clone());
                map.insert(pis.clone(), pis.clone());

                for key in ["code", "id", "registration", "bars", "rfid"] {
                    if let Some(raw_key) = value_to_string(key) {
                        let normalized_key = Self::normalize_code(&raw_key);
                        if !normalized_key.is_empty() && normalized_key != "0" {
                            map.insert(normalized_key, pis.clone());
                        }
                    }
                }
            }

            offset += users.len() as u64;

            if let Some(count) = total_count {
                if offset >= count {
                    break;
                }
            }

            if users.len() < limit as usize {
                break;
            }
        }

        let mut ambiguous_prefixes = HashSet::new();
        for pis in &pis_values {
            if pis.len() < 5 {
                continue;
            }
            let prefix = pis[..5].to_string();
            if let Some(existing) = map.get(&prefix) {
                if *existing != *pis {
                    ambiguous_prefixes.insert(prefix.clone());
                }
            } else {
                map.insert(prefix, pis.clone());
            }
        }

        for prefix in ambiguous_prefixes {
            map.remove(&prefix);
        }

        let mut suffix_candidates: HashMap<String, String> = HashMap::new();
        let mut ambiguous_suffixes = HashSet::new();
        for pis in &pis_values {
            if pis.len() < 10 {
                continue;
            }

            let suffix = pis[pis.len() - 10..].to_string();
            if let Some(existing) = suffix_candidates.get(&suffix) {
                if existing != pis {
                    ambiguous_suffixes.insert(suffix.clone());
                }
            } else {
                suffix_candidates.insert(suffix, pis.clone());
            }
        }

        for suffix in ambiguous_suffixes {
            suffix_candidates.remove(&suffix);
        }

        for (suffix, pis) in suffix_candidates {
            map.entry(suffix).or_insert(pis);
        }

        Ok(map)
    }

    pub fn parse_afd(afd_text: &str) -> Vec<PunchRecord> {
        let mut records = Vec::new();
        
        for line in afd_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("AFD") {
                continue;
            }

            // Linha tipo 3 = marcação de ponto (Portaria MTE 595/2007)
            // Formato (0-indexed):
            //   0..9  = NSR (9 dígitos)
            //   9     = tipo de linha ('3' = marcação de ponto)
            //  10..18 = data DDMMAAAA
            //  18..22 = hora HHMM
            //  22     = direção (0=desconhecido, 1=entrada, 2=saída)
            //  23..34 = PIS/PASEP (11 dígitos)
            //  34..38 = CRC (4 hex)
            if line.len() < 34 {
                log::warn!("Skipping malformed AFD line (len={}): {}", line.len(), line);
                continue;
            }

            // Só processa linhas de marcação de ponto (tipo 3)
            if line.get(9..10) != Some("3") {
                continue;
            }

            // NSR: primeiros 9 caracteres
            let nsr: u64 = line.get(0..9)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            let Some(date_raw) = line.get(10..18) else { continue; }; // DDMMAAAA
            let Some(time_raw) = line.get(18..22) else { continue; }; // HHMM
            let Some(direction_raw) = line.get(22..23) else { continue; };
            let Some(pis_raw) = line.get(23..34) else { continue; };

            // Parseia DDMMAAAA + HHMM diretamente das posições fixas
            let timestamp = match (
                date_raw.get(0..2).and_then(|s| s.parse::<u32>().ok()), // DD
                date_raw.get(2..4).and_then(|s| s.parse::<u32>().ok()), // MM
                date_raw.get(4..8).and_then(|s| s.parse::<i32>().ok()), // AAAA
                time_raw.get(0..2).and_then(|s| s.parse::<u32>().ok()), // HH
                time_raw.get(2..4).and_then(|s| s.parse::<u32>().ok()), // MM
            ) {
                (Some(d), Some(m), Some(y), Some(hh), Some(mm))
                    if y >= 2000 && y <= 2100 =>
                {
                    use chrono::NaiveDateTime;
                    match NaiveDateTime::parse_from_str(
                        &format!("{:02}{:02}{:04}{:02}{:02}00", d, m, y, hh, mm),
                        "%d%m%Y%H%M%S",
                    ) {
                        Ok(dt) => dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                        Err(_) => {
                            log::warn!("Skipping AFD line with invalid date fields: {}", line);
                            continue;
                        }
                    }
                }
                _ => {
                    log::warn!("Skipping AFD line with unparseable date: {}", line);
                    continue;
                }
            };

            let record_type = match direction_raw {
                "1" => RecordType::ClockIn,
                "2" => RecordType::ClockOut,
                _ => RecordType::Unknown,
            };

            let employee_code = pis_raw.trim_start_matches('0').to_string();
            if employee_code.is_empty() {
                log::warn!("Skipping AFD line with empty PIS: {}", line);
                continue;
            }

            records.push(PunchRecord {
                employee_code,
                timestamp,
                record_type,
                nsr,
                raw_line: line.to_string(),
            });
        }
        
        records
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SystemInfo {
    pub user_count: u32,
    pub template_count: u32,
    pub last_nsr: u64,
}

pub async fn login(ip: &str, user: &str, password: &str) -> Result<String, String> {
    let mut client = IdClassClient::new(ip);
    client.login(user, password).await
}

pub async fn get_records(ip: &str, user: &str, password: &str, last_nsr: u64) -> Result<RecordsBatch, String> {
    let mut client = IdClassClient::new(ip);
    client.login(user, password).await?;
    let system_info = client.get_system_info().await?;
    let afd = client.get_afd(last_nsr).await?;
    Ok(RecordsBatch {
        records: IdClassClient::parse_afd(&afd),
        latest_nsr: system_info.last_nsr,
    })
}

pub async fn get_user_pis_mappings(
    ip: &str,
    user: &str,
    password: &str,
) -> Result<HashMap<String, String>, String> {
    let mut client = IdClassClient::new(ip);
    client.login(user, password).await?;
    client.load_user_pis_map().await
}
