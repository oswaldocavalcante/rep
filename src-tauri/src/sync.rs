use crate::collector;
use crate::config::Config;
use crate::idclass;
use crate::state;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn normalize_code(value: &str) -> String {
    let digits_only: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits_only.is_empty() {
        return value.trim().to_string();
    }
    let trimmed = digits_only.trim_start_matches('0');
    if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub success: bool,
    pub records_sent: u32,
    pub message: String,
}

pub async fn sync(config: &Config) -> Result<SyncResult, String> {
    log::info!("Starting sync...");
    let _ = state::save_log("info", 0, "Iniciando sincronização");

    let current_state = state::load_state().map_err(|e| e.to_string())?;
    let last_nsr = current_state.last_nsr;

    log::info!("Fetching records since NSR {}", last_nsr);
    let _ = state::save_log("info", 0, &format!("Buscando registros desde NSR {}", last_nsr));

    let records_batch = idclass::get_records(
        &config.device_ip,
        &config.device_user,
        &config.device_password,
        last_nsr,
    )
    .await;

    let result = match records_batch {
        Ok(batch) => {
            let auto_mappings = match idclass::get_user_pis_mappings(
                &config.device_ip,
                &config.device_user,
                &config.device_password,
            )
            .await
            {
                Ok(found) => {
                    log::info!("Loaded {} user mappings from device", found.len());
                    found
                }
                Err(error) => {
                    log::warn!("Could not load users from device for automatic mapping: {}", error);
                    HashMap::new()
                }
            };

            let mut auto_applied = 0u32;
            let recs: Vec<_> = batch
                .records
                .into_iter()
                .map(|mut record| {
                    let current = normalize_code(&record.employee_code);
                    if let Some(mapped) = auto_mappings.get(&current) {
                        record.employee_code = mapped.clone();
                        auto_applied += 1;
                    } else {
                        record.employee_code = current;
                    }
                    record
                })
                .collect();

            let mapping_msg = format!(
                "Mapeamentos aplicados: automático={}, total_chaves_mapeadas={}",
                auto_applied,
                auto_mappings.len()
            );
            let _ = state::save_log("info", 0, &mapping_msg);

            if recs.is_empty() {
                log::info!("No new records to sync");
                let _ = state::save_log("success", 0, "Nenhum registro novo");
                return Ok(SyncResult {
                    success: true,
                    records_sent: 0,
                    message: "Nenhum registro novo".to_string(),
                });
            }

            log::info!("Found {} new records", recs.len());
            let _ = state::save_log("info", recs.len() as u32, &format!("{} registros encontrados", recs.len()));

            let preview_records: Vec<_> = recs.iter().take(20).cloned().collect();
            let preview_json = serde_json::to_string_pretty(&preview_records)
                .unwrap_or_else(|_| "[\"erro ao serializar preview\"]".to_string());
            let preview_message = if recs.len() > 20 {
                format!(
                    "COLETA_PREVIEW total={} exibindo=20\n{}\n... (truncado)",
                    recs.len(),
                    preview_json
                )
            } else {
                format!("COLETA_PREVIEW total={}\n{}", recs.len(), preview_json)
            };
            let _ = state::save_log("info", recs.len() as u32, &preview_message);

            let send_result = collector::send_records(&config.app_url, &config.api_key, recs.clone()).await;

            match send_result {
                Ok(send_stats) => {
                    let new_state = state::State {
                        last_synced_at: Utc::now(),
                        last_nsr: batch.latest_nsr,
                    };
                    state::save_state(&new_state).map_err(|e| e.to_string())?;

                    let msg = format!(
                        "Importação concluída: inseridos={}, duplicados={}, ignorados={} (recebidos={})",
                        send_stats.inserted,
                        send_stats.duplicates,
                        send_stats.ignored,
                        send_stats.received
                    );
                    let _ = state::save_log("success", send_stats.inserted, &msg);
                    
                    log::info!("Sync completed successfully");
                    
                    Ok(SyncResult {
                        success: true,
                        records_sent: send_stats.inserted,
                        message: msg,
                    })
                }
                Err(e) => {
                    let _ = state::save_log("error", 0, &e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            let _ = state::save_log("error", 0, &e);
            Err(e)
        }
    };

    result
}
