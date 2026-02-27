use crate::collector;
use crate::config::Config;
use crate::idclass;
use crate::state;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub success: bool,
    pub records_sent: u32,
    pub message: String,
}

pub async fn sync(config: &Config) -> Result<SyncResult, String> {
    log::info!("Starting sync...");

    let current_state = state::load_state().map_err(|e| e.to_string())?;
    let last_nsr = current_state.last_nsr;

    log::info!("Fetching records since NSR {}", last_nsr);

    let records_batch = idclass::get_records(
        &config.device_ip,
        &config.device_user,
        &config.device_password,
        last_nsr,
    )
    .await;

    let result = match records_batch {
        Ok(batch) => {
            let recs = batch.records;
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

            let send_result = collector::send_records(&config.app_url, &config.api_key, recs.clone()).await;

            match send_result {
                Ok(()) => {
                    let new_state = state::State {
                        last_synced_at: Utc::now(),
                        last_nsr: batch.latest_nsr,
                    };
                    state::save_state(&new_state).map_err(|e| e.to_string())?;
                    
                    let msg = format!("{} registros sincronizados", recs.len());
                    let _ = state::save_log("success", recs.len() as u32, &msg);
                    
                    log::info!("Sync completed successfully");
                    
                    Ok(SyncResult {
                        success: true,
                        records_sent: recs.len() as u32,
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
