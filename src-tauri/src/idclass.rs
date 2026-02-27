use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PunchRecord {
    pub employee_code: String,
    pub timestamp: String,
    pub record_type: RecordType,
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

impl IdClassClient {
    pub fn new(ip: &str) -> Self {
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
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
        log::info!("Logged in to IDClass, session: {}", &session[..8]);
        
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

        let text = response.text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        Ok(text)
    }

    pub fn parse_afd(afd_text: &str) -> Vec<PunchRecord> {
        let mut records = Vec::new();
        
        for line in afd_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("AFD") {
                continue;
            }
            
            if line.len() >= 32 {
                let nsr = &line[0..9].trim_start_matches('0');
                let date = &line[9..17];
                let time = &line[17..23];
                let code = &line[23..28].trim_start_matches('0');
                let record_type = match line.chars().nth(28) {
                    Some('1') => RecordType::ClockIn,
                    Some('2') => RecordType::ClockOut,
                    _ => RecordType::Unknown,
                };
                let pis = &line[29..41].trim_start_matches('0');
                
                let day = &date[0..2];
                let month = &date[2..4];
                let year = &date[4..8];
                let hour = &time[0..2];
                let minute = &time[2..4];
                let second = &time[4..6];
                
                let timestamp = format!("{}-{}-{}T{}:{}:{}Z", year, month, day, hour, minute, second);
                
                records.push(PunchRecord {
                    employee_code: if code.is_empty() { pis.to_string() } else { code.to_string() },
                    timestamp,
                    record_type,
                });
            }
        }
        
        records
    }
}

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

pub async fn get_records(ip: &str, user: &str, password: &str, last_nsr: u64) -> Result<Vec<PunchRecord>, String> {
    let mut client = IdClassClient::new(ip);
    client.login(user, password).await?;
    let afd = client.get_afd(last_nsr).await?;
    Ok(IdClassClient::parse_afd(&afd))
}
