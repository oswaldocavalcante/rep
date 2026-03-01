use crate::collector;
use crate::config::{Config, RYANNE_API_URL};
use crate::idclass;
use crate::state;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

fn normalize_code(value: &str) -> String {
    let digits_only: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits_only.is_empty() {
        return value.trim().to_string();
    }
    let trimmed = digits_only.trim_start_matches('0');
    if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
}

/// Reporta o status da última sincronização ao endpoint /api/time-clocks/:id/sync-status.
/// Falha silenciosamente para não interromper o fluxo principal.
async fn report_sync_status(api_key: &str, clock_id: &str, last_sync_at: Option<&str>, last_error: Option<&str>) {
    if clock_id.is_empty() || api_key.is_empty() {
        return;
    }
    let url = format!("{}/api/time-clocks/{}/sync-status", RYANNE_API_URL, clock_id);
    let mut body = serde_json::json!({});
    if let Some(ts) = last_sync_at {
        body["lastSyncAt"] = serde_json::Value::String(ts.to_string());
    }
    if let Some(err) = last_error {
        body["lastError"] = serde_json::Value::String(err.to_string());
    }
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await;
}

/// Busca as credenciais do dispositivo REP via sistema.
/// Retorna (ipAddress, deviceUser, devicePassword).
pub async fn fetch_device_credentials(
    api_key: &str,
    clock_id: &str,
) -> Result<(String, String, String), String> {
    if api_key.is_empty() || clock_id.is_empty() {
        return Err("api_key e clock_id são obrigatórios".to_string());
    }
    let url = format!("{}/api/time-clocks/{}/config", RYANNE_API_URL, clock_id);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Erro de rede ao buscar config: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Servidor retornou {}: {}", status, body));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Erro ao parsear resposta: {}", e))?;

    let ip = json
        .get("ipAddress")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let user = json
        .get("deviceUser")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let password = json
        .get("devicePassword")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if ip.is_empty() {
        return Err("ipAddress não encontrado na resposta do sistema".to_string());
    }

    Ok((ip, user, password))
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

    // Auto-provisionamento: se device_ip vazio mas clock_id+api_key presentes, busca credenciais
    let config_owned;
    let config = if config.device_ip.is_empty()
        && !config.clock_id.is_empty()
        && !config.api_key.is_empty()
    {
        log::info!("device_ip vazio, tentando provisionamento automático via sistema...");
        let _ = state::save_log("info", 0, "Buscando credenciais do dispositivo no sistema...");
        match fetch_device_credentials(&config.api_key, &config.clock_id).await {
            Ok((ip, user, password)) => {
                let mut c = config.clone();
                c.device_ip = ip.clone();
                c.device_user = user;
                c.device_password = password;
                // Persiste as credenciais obtidas
                if let Err(e) = crate::config::save_config(&c) {
                    log::warn!("Erro ao persistir credenciais provisionadas: {}", e);
                } else {
                    log::info!("Credenciais provisionadas e salvas: ip={}", ip);
                }
                config_owned = c;
                &config_owned
            }
            Err(e) => {
                let msg = format!("Provisionamento falhou: {}", e);
                let _ = state::save_log("error", 0, &msg);
                return Err(msg);
            }
        }
    } else {
        config
    };


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

            let allowed_codes = collector::fetch_allowed_employee_codes(&config.api_key).await;
            let allowed_codes = match allowed_codes {
                Ok(codes) => codes,
                Err(error) => {
                    let _ = state::save_log("error", 0, &format!("Falha ao buscar colaboradores ativos no app: {}", error));
                    return Err(error);
                }
            };
            let allowed_codes_normalized: std::collections::HashSet<String> = allowed_codes
                .iter()
                .map(|code| normalize_code(code))
                .collect();

            let before_filter = recs.len();
            let recs: Vec<_> = recs
                .into_iter()
                .filter(|record| {
                    let normalized = normalize_code(&record.employee_code);
                    allowed_codes_normalized.contains(&normalized)
                })
                .collect();

            let filtered_out = before_filter.saturating_sub(recs.len());
            let _ = state::save_log(
                "info",
                recs.len() as u32,
                &format!(
                    "Filtro por colaboradores ativos aplicado: permitidos={}, descartados={}",
                    recs.len(),
                    filtered_out
                )
            );

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

            let send_result = collector::send_records(&config.api_key, &config.clock_id, recs.clone()).await;

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

                    let ts = Utc::now().to_rfc3339();
                    report_sync_status(&config.api_key, &config.clock_id, Some(&ts), None).await;
                    
                    Ok(SyncResult {
                        success: true,
                        records_sent: send_stats.inserted,
                        message: msg,
                    })
                }
                Err(e) => {
                    let _ = state::save_log("error", 0, &e);
                    report_sync_status(&config.api_key, &config.clock_id, None, Some(&e)).await;
                    Err(e)
                }
            }
        }
        Err(e) => {
            let _ = state::save_log("error", 0, &e);
            report_sync_status(&config.api_key, &config.clock_id, None, Some(&e)).await;
            Err(e)
        }
    };

    result
}
