use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::auth::{hash_password, load_auth_config, save_auth_config, AuthConfig, TokenStore};
use crate::{config, idclass, state, sync};

// ─── Estado compartilhado da aplicação ───────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub sync_lock: Arc<Mutex<()>>,
    pub token_store: TokenStore,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sync_lock: Arc::new(Mutex::new(())),
            token_store: TokenStore::new(),
        }
    }
}

// ─── Middleware de autenticação ───────────────────────────────────────────────

pub async fn require_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if !state.token_store.validate_token(token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }

    next.run(request).await
}

// ─── Handlers públicos ────────────────────────────────────────────────────────

async fn health_handler() -> impl IntoResponse {
    Json(json!({"status": "ok", "service": "rep-server"}))
}

#[derive(Deserialize)]
struct LoginRequest {
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

async fn login_handler(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let auth = load_auth_config();
    if hash_password(&body.password) != auth.password_hash {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Senha incorreta"})),
        )
            .into_response();
    }
    let token = state.token_store.create_token();
    Json(LoginResponse { token }).into_response()
}

// ─── Handlers protegidos ──────────────────────────────────────────────────────

async fn logout_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        state.token_store.revoke_token(token);
    }
    Json(json!({"success": true}))
}

async fn me_handler() -> impl IntoResponse {
    Json(json!({"authenticated": true}))
}

async fn status_handler() -> impl IntoResponse {
    let current_state = match state::load_state() {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    let logs = match state::load_logs() {
        Ok(l) => l,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    let cfg = match config::load_config() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let last_synced_at = if current_state.last_synced_at == chrono::DateTime::<chrono::Utc>::MIN_UTC {
        None::<String>
    } else {
        Some(current_state.last_synced_at.to_rfc3339())
    };

    let (last_records_sent, last_message) = logs
        .entries
        .iter()
        .find(|e| e.status != "info")
        .map(|e| (e.records_sent, e.message.clone()))
        .unwrap_or((0, String::new()));

    let next_sync_at = last_synced_at.as_ref().and_then(|v| {
        chrono::DateTime::parse_from_rfc3339(v)
            .ok()
            .map(|dt| {
                (dt + chrono::Duration::seconds(cfg.sync_interval_secs as i64))
                    .to_utc()
                    .to_rfc3339()
            })
    });

    Json(json!({
        "last_synced_at": last_synced_at,
        "last_nsr": current_state.last_nsr,
        "last_records_sent": last_records_sent,
        "last_message": last_message,
        "sync_interval_secs": cfg.sync_interval_secs,
        "next_sync_at": next_sync_at,
    }))
    .into_response()
}

async fn get_config_handler() -> impl IntoResponse {
    match config::load_config() {
        Ok(mut c) => {
            // Mascara a senha do dispositivo
            if !c.device_password.is_empty() {
                c.device_password = "••••••••".to_string();
            }
            Json(serde_json::to_value(c).unwrap()).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ConfigUpdate {
    device_ip: Option<String>,
    device_user: Option<String>,
    device_password: Option<String>,
    app_url: Option<String>,
    api_key: Option<String>,
    clock_id: Option<String>,
    sync_interval_secs: Option<u64>,
}

async fn put_config_handler(Json(body): Json<ConfigUpdate>) -> impl IntoResponse {
    let mut c = match config::load_config() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    if let Some(v) = body.device_ip {
        c.device_ip = v;
    }
    if let Some(v) = body.device_user {
        c.device_user = v;
    }
    if let Some(v) = body.device_password {
        // Não sobrescreve se for placeholder
        if !v.starts_with('•') {
            c.device_password = v;
        }
    }
    if let Some(v) = body.app_url {
        c.app_url = v;
    }
    if let Some(v) = body.api_key {
        c.api_key = v;
    }
    if let Some(v) = body.clock_id {
        c.clock_id = v;
    }
    if let Some(v) = body.sync_interval_secs {
        c.sync_interval_secs = v;
    }

    match config::save_config(&c) {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ProvisionRequest {
    app_url: String,
    api_key: String,
    clock_id: String,
}

async fn provision_handler(Json(body): Json<ProvisionRequest>) -> impl IntoResponse {
    let mut c = config::load_config().unwrap_or_default();
    c.app_url = body.app_url.clone();
    c.api_key = body.api_key.clone();
    c.clock_id = body.clock_id.clone();

    match sync::fetch_device_credentials(&c.app_url, &c.api_key, &c.clock_id).await {
        Ok((ip, user, password)) => {
            c.device_ip = ip.clone();
            c.device_user = user;
            c.device_password = password;
            match config::save_config(&c) {
                Ok(_) => Json(json!({"success": true, "ipAddress": ip})).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response(),
            }
        }
        Err(e) => {
            // Salva app_url/api_key/clock_id mesmo sem credenciais do dispositivo
            let _ = config::save_config(&c);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("Erro ao buscar config do dispositivo: {}", e)})),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
struct TestConnectionRequest {
    device_ip: String,
    device_user: String,
    device_password: String,
}

async fn test_connection_handler(Json(body): Json<TestConnectionRequest>) -> impl IntoResponse {
    match idclass::login(&body.device_ip, &body.device_user, &body.device_password).await {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"success": false, "error": e})),
        )
            .into_response(),
    }
}

async fn sync_run_handler(State(app_state): State<AppState>) -> impl IntoResponse {
    match app_state.sync_lock.try_lock() {
        Ok(_guard) => {
            let cfg = match config::load_config() {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": e.to_string()})),
                    )
                        .into_response()
                }
            };
            match sync::sync(&cfg).await {
                Ok(result) => Json(serde_json::to_value(result).unwrap()).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e})),
                )
                    .into_response(),
            }
        }
        Err(_) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Sincronização já em andamento"})),
        )
            .into_response(),
    }
}

async fn sync_reset_handler(State(app_state): State<AppState>) -> impl IntoResponse {
    match app_state.sync_lock.try_lock() {
        Ok(_guard) => {
            match state::save_state(&state::State::default()) {
                Ok(_) => {
                    let _ = state::save_log(
                        "success",
                        0,
                        "Cursor de sincronização resetado manualmente (NSR=0)",
                    );
                    Json(json!({"success": true})).into_response()
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response(),
            }
        }
        Err(_) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Sincronização já em andamento"})),
        )
            .into_response(),
    }
}

async fn sync_reprocess_handler(State(app_state): State<AppState>) -> impl IntoResponse {
    match app_state.sync_lock.try_lock() {
        Ok(_guard) => {
            if let Err(e) = state::save_state(&state::State::default()) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response();
            }
            let _ = state::save_log(
                "success",
                0,
                "Cursor de sincronização resetado para reprocessamento (NSR=0)",
            );
            let cfg = match config::load_config() {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": e.to_string()})),
                    )
                        .into_response()
                }
            };
            match sync::sync(&cfg).await {
                Ok(result) => Json(serde_json::to_value(result).unwrap()).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e})),
                )
                    .into_response(),
            }
        }
        Err(_) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "Sincronização já em andamento"})),
        )
            .into_response(),
    }
}

async fn logs_handler() -> impl IntoResponse {
    match state::load_logs() {
        Ok(logs) => Json(logs.entries).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

async fn change_password_handler(Json(body): Json<ChangePasswordRequest>) -> impl IntoResponse {
    let auth = load_auth_config();
    if hash_password(&body.current_password) != auth.password_hash {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Senha atual incorreta"})),
        )
            .into_response();
    }
    let new_auth = AuthConfig {
        password_hash: hash_password(&body.new_password),
    };
    match save_auth_config(&new_auth) {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ─── Router ───────────────────────────────────────────────────────────────────

/// Cria o router axum completo.
/// `web_dir` é o caminho para os arquivos estáticos compilados da UI
/// (ex: `/usr/share/rep/web`). Se None, o fallback de arquivos não é registrado.
pub fn create_router(state: AppState, web_dir: Option<String>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Rotas públicas
    let public_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/auth/login", post(login_handler));

    // Rotas protegidas por Bearer token
    let protected_routes = Router::new()
        .route("/auth/logout", post(logout_handler))
        .route("/auth/me", get(me_handler))
        .route("/api/status", get(status_handler))
        .route(
            "/api/config",
            get(get_config_handler).put(put_config_handler),
        )
        .route("/api/provision", post(provision_handler))
        .route("/api/test-connection", post(test_connection_handler))
        .route("/api/sync/run", post(sync_run_handler))
        .route("/api/sync/reset", post(sync_reset_handler))
        .route("/api/sync/reprocess", post(sync_reprocess_handler))
        .route("/api/logs", get(logs_handler))
        .route("/api/auth/password", put(change_password_handler))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    let mut router = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(cors)
        .with_state(state);

    // Serve UI estática no modo LXC
    if let Some(dir) = web_dir {
        let index = format!("{}/index.html", dir);
        router = router
            .nest_service("/assets", ServeDir::new(format!("{}/assets", dir)))
            .fallback_service(ServeFile::new(index));
    }

    router
}
