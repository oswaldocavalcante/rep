use crate::config::RYANNE_API_URL;
use crate::idclass::PunchRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Serialize)]
struct PunchRequest {
    #[serde(rename = "clockId", skip_serializing_if = "Option::is_none")]
    clock_id: Option<String>,
    records: Vec<PunchRecordDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendStats {
    pub received: u32,
    pub inserted: u32,
    pub duplicates: u32,
    pub ignored: u32,
    #[serde(default)]
    pub errors: Vec<CollectorError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollectorError {
    #[serde(rename = "employeeCode")]
    pub employee_code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
struct PunchRecordDto {
    #[serde(rename = "employeeCode")]
    employee_code: String,
    timestamp: String,
    #[serde(rename = "type")]
    record_type: String,
    #[serde(rename = "externalNsr", skip_serializing_if = "Option::is_none")]
    external_nsr: Option<String>,
    #[serde(rename = "rawPayload", skip_serializing_if = "Option::is_none")]
    raw_payload: Option<String>,
}

pub async fn send_records(
    api_key: &str,
    clock_id: &str,
    records: Vec<PunchRecord>,
) -> Result<SendStats, String> {
    if records.is_empty() {
        log::info!("No records to send");
        return Ok(SendStats {
            received: 0,
            inserted: 0,
            duplicates: 0,
            ignored: 0,
            errors: vec![],
        });
    }

    let url = format!("{}/api/punch-collector", RYANNE_API_URL);
    let clock_id_opt: Option<String> = if clock_id.is_empty() { None } else { Some(clock_id.to_string()) };
    
    let request_records: Vec<PunchRecordDto> = records
        .into_iter()
        .map(|r| {
            let external_nsr = if r.nsr > 0 && !clock_id.is_empty() {
                Some(format!("{}:{}", clock_id, r.nsr))
            } else {
                None
            };
            let raw_payload = if r.raw_line.is_empty() { None } else { Some(r.raw_line.clone()) };
            PunchRecordDto {
                employee_code: r.employee_code,
                timestamp: r.timestamp,
                record_type: match r.record_type {
                    crate::idclass::RecordType::ClockIn => "CLOCK_IN".to_string(),
                    crate::idclass::RecordType::ClockOut => "CLOCK_OUT".to_string(),
                    crate::idclass::RecordType::Unknown => "UNKNOWN".to_string(),
                },
                external_nsr,
                raw_payload,
            }
        })
        .collect();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let chunk_size = 500usize;
    let total_chunks = request_records.len().div_ceil(chunk_size);

    let mut total = SendStats {
        received: 0,
        inserted: 0,
        duplicates: 0,
        ignored: 0,
        errors: vec![],
    };

    for (index, chunk) in request_records.chunks(chunk_size).enumerate() {
        let body = PunchRequest {
            clock_id: clock_id_opt.clone(),
            records: chunk.to_vec(),
        };

        let stats = send_chunk_with_retry(&client, &url, api_key, &body)
            .await
            .map_err(|e| format!("Chunk {}/{} failed: {}", index + 1, total_chunks, e))?;

        total.received += stats.received;
        total.inserted += stats.inserted;
        total.duplicates += stats.duplicates;
        total.ignored += stats.ignored;
        total.errors.extend(stats.errors);
    }

    log::info!(
        "Records sent in {} chunks: received={}, inserted={}, duplicates={}, ignored={}",
        total_chunks,
        total.received,
        total.inserted,
        total.duplicates,
        total.ignored
    );

    Ok(total)
}

#[derive(Debug, Deserialize)]
struct EmployeeCodesResponse {
    #[serde(rename = "employeeCodes")]
    employee_codes: Vec<String>,
}

pub async fn fetch_allowed_employee_codes(
    api_key: &str,
) -> Result<HashSet<String>, String> {
    let url = format!("{}/api/punch-collector/employees", RYANNE_API_URL);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Connection error while fetching employees: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to fetch employees whitelist: {} {}", status, text));
    }

    let payload: EmployeeCodesResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse employees whitelist response: {}", e))?;

    Ok(payload.employee_codes.into_iter().collect())
}

async fn send_chunk_with_retry(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    body: &PunchRequest,
) -> Result<SendStats, String> {

    let mut retries = 3;
    let mut last_error = String::new();

    while retries > 0 {
        let result = client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await;

        match result {
            Ok(response) => {
                if response.status().is_success() {
                    let stats: SendStats = response
                        .json()
                        .await
                        .map_err(|e| format!("Failed to parse server response: {}", e))?;
                    // Loga primeiros 5 erros para diagnóstico
                    if !stats.errors.is_empty() {
                        let sample: Vec<String> = stats.errors.iter().take(5)
                            .map(|e| format!("{}: {}", e.employee_code, e.message))
                            .collect();
                        log::warn!("Ignored records sample: {}", sample.join(" | "));
                        let _ = crate::state::save_log(
                            "warn",
                            stats.ignored,
                            &format!("Registros ignorados (amostra): {}", sample.join(" | ")),
                        );
                    }
                    return Ok(stats);
                } else {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    last_error = format!("Server returned {}: {}", status, text);
                }
            }
            Err(e) => {
                last_error = format!("Connection error: {}", e);
            }
        }

        retries -= 1;
        if retries > 0 {
            log::warn!("Failed to send records, retrying in 5 seconds... ({}/3)", 3 - retries);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    Err(format!("Failed to send records after 3 attempts: {}", last_error))
}

use std::time::Duration;
