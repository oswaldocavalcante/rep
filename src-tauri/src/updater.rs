use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const GITHUB_REPO: &str = "oswaldocavalcante/rep";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

/// Consulta a GitHub Releases API e retorna informações de versão.
pub async fn check_update() -> Result<VersionInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("ryanne-ponto-rep-server/{}", CURRENT_VERSION))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let release: GithubRelease = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Erro ao consultar GitHub: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Erro ao parsear resposta: {}", e))?;

    let latest = release.tag_name.trim_start_matches('v').to_string();
    let current = CURRENT_VERSION.to_string();
    let update_available = is_newer(&latest, &current);

    Ok(VersionInfo {
        current_version: current,
        latest_version: latest,
        update_available,
        release_url: release.html_url,
    })
}

/// Compara versões semânticas. Retorna true se `latest` > `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u64, u64, u64) {
        let parts: Vec<&str> = v.split('.').collect();
        let n = |i: usize| -> u64 {
            parts.get(i).and_then(|s| s.parse().ok()).unwrap_or(0)
        };
        (n(0), n(1), n(2))
    };
    parse(latest) > parse(current)
}
