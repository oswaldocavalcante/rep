use crate::idclass::PunchRecord;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct PunchRequest {
    records: Vec<PunchRecordDto>,
}

#[derive(Debug, Serialize)]
struct PunchRecordDto {
    #[serde(rename = "employeeCode")]
    employee_code: String,
    timestamp: String,
    #[serde(rename = "type")]
    record_type: String,
}

pub async fn send_records(
    app_url: &str,
    api_key: &str,
    records: Vec<PunchRecord>,
) -> Result<(), String> {
    if records.is_empty() {
        log::info!("No records to send");
        return Ok(());
    }

    let url = format!("{}/api/punch-collector", app_url);
    
    let request_records: Vec<PunchRecordDto> = records
        .into_iter()
        .map(|r| PunchRecordDto {
            employee_code: r.employee_code,
            timestamp: r.timestamp,
            record_type: match r.record_type {
                crate::idclass::RecordType::ClockIn => "CLOCK_IN".to_string(),
                crate::idclass::RecordType::ClockOut => "CLOCK_OUT".to_string(),
                crate::idclass::RecordType::Unknown => "UNKNOWN".to_string(),
            },
        })
        .collect();

    let body = PunchRequest {
        records: request_records,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let mut retries = 3;
    let mut last_error = String::new();

    while retries > 0 {
        let result = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await;

        match result {
            Ok(response) => {
                if response.status().is_success() {
                    log::info!("Records sent successfully");
                    return Ok(());
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
