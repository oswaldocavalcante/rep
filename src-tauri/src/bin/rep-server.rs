use ryanne_ponto_lib::server::{AppState, create_router};
use ryanne_ponto_lib::sync;
use ryanne_ponto_lib::config;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    // Inicializa logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Iniciando rep-server...");

    // Configuração via variáveis de ambiente
    let port: u16 = std::env::var("REP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let web_dir: Option<String> = std::env::var("REP_WEB_DIR").ok().filter(|v| !v.is_empty());

    // Provisionamento automático na inicialização (se variáveis de ambiente fornecidas)
    let env_api_key = std::env::var("REP_API_KEY").unwrap_or_default();
    let env_clock_id = std::env::var("REP_CLOCK_ID").unwrap_or_default();

    if !env_api_key.is_empty() && !env_clock_id.is_empty() {
        log::info!("Variáveis de provisionamento detectadas, verificando config...");
        let mut cfg = config::load_config().unwrap_or_default();

        // Atualiza campos de provisionamento se diferentes do atual
        let needs_update = cfg.api_key != env_api_key
            || cfg.clock_id != env_clock_id;

        if needs_update {
            cfg.api_key = env_api_key.clone();
            cfg.clock_id = env_clock_id.clone();

            log::info!("Buscando credenciais do dispositivo via sistema...");
            match sync::fetch_device_credentials(&env_api_key, &env_clock_id).await {
                Ok((ip, user, password)) => {
                    cfg.device_ip = ip;
                    cfg.device_user = user;
                    cfg.device_password = password;
                    log::info!("Credenciais do dispositivo obtidas com sucesso.");
                }
                Err(e) => {
                    log::warn!("Não foi possível obter credenciais do dispositivo: {}. Configure via UI.", e);
                }
            }

            if let Err(e) = config::save_config(&cfg) {
                log::error!("Erro ao salvar config de provisionamento: {}", e);
            }
        }
    }

    // Estado compartilhado
    let state = AppState::new();

    // Loop de sync em background
    let bg_state = state.clone();
    tokio::spawn(async move {
        loop {
            let cfg = match config::load_config() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Erro ao carregar config para sync: {}", e);
                    sleep(Duration::from_secs(60)).await;
                    continue;
                }
            };

            if cfg.device_ip.is_empty() || cfg.api_key.is_empty() {
                log::debug!("Config incompleta, aguardando...");
                sleep(Duration::from_secs(30)).await;
                continue;
            }

            let interval = cfg.sync_interval_secs;

            // Tenta adquirir o lock sem bloquear (não executa se sync manual em andamento)
            if let Ok(_guard) = bg_state.sync_lock.try_lock() {
                log::info!("Executando sync automático...");
                match sync::sync(&cfg).await {
                    Ok(result) => log::info!("Sync automático: {}", result.message),
                    Err(e) => log::error!("Erro no sync automático: {}", e),
                }
            } else {
                log::debug!("Sync manual em andamento, pulando sync automático.");
            }

            sleep(Duration::from_secs(interval)).await;
        }
    });

    // Inicia o servidor HTTP
    let addr = format!("0.0.0.0:{}", port);
    log::info!("Servindo na porta {} | UI: {:?}", port, web_dir);

    let router = create_router(state, web_dir);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Falha ao iniciar listener em {}: {}", addr, e));

    axum::serve(listener, router)
        .await
        .expect("Falha ao iniciar servidor axum");
}
