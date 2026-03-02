#!/usr/bin/env bash
# ==============================================================================
# rep-update.sh — CLI e auto-updater do Ryanne REP
#
# Uso:
#   rep-ctl version          Exibe versão atual e a mais recente no GitHub
#   rep-ctl check            Verifica se há atualização (exit 0 = tem, 1 = não)
#   rep-ctl update           Aplica atualização (se houver) e reinicia o serviço
#   rep-ctl update --force   Força reinstalação mesmo se já na versão mais recente
#
# Instalado como /usr/local/bin/rep-ctl pelo install-lxc.sh.
# Também usado pelo rep-update.service (modo automático).
# ==============================================================================
set -euo pipefail

REPO="oswaldocavalcante/rep"
BINARY_SRC="rep-server-linux-x86_64"
BINARY_DEST="/usr/local/bin/rep-server"
WEB_DEST="/usr/share/rep/web"
SERVICE_NAME="rep-server"
GH_API="https://api.github.com/repos/${REPO}/releases/latest"

# ── Helpers ───────────────────────────────────────────────────────────────────
info()    { echo "[rep-ctl] $*"; }
success() { echo "[rep-ctl] ✓ $*"; }
warn()    { echo "[rep-ctl] ⚠ $*" >&2; }
error()   { echo "[rep-ctl] ERRO: $*" >&2; exit 1; }

current_version() {
    "$BINARY_DEST" --version 2>/dev/null \
        | grep -oP '\d+\.\d+\.\d+' | head -1 \
        || echo "0.0.0"
}

fetch_release_json() {
    curl -fsSL --max-time 10 \
        -H "Accept: application/vnd.github+json" \
        "$GH_API"
}

latest_version() {
    fetch_release_json | grep -o '"tag_name": *"[^"]*"' \
        | grep -oP 'v?\K\d+\.\d+\.\d+' | head -1
}

asset_url() {
    local name="$1"
    fetch_release_json \
        | grep -o '"browser_download_url": *"[^"]*'"$name"'[^"]*"' \
        | grep -o 'https://[^"]*' | head -1
}

semver_gt() {
    # Retorna 0 (true) se $1 > $2
    local IFS=.
    local a=($1) b=($2)
    for i in 0 1 2; do
        local ai bi
        # Remove caracteres não-numéricos para evitar erro em contexto aritmético
        ai="${a[$i]:-0}"; ai="${ai//[^0-9]/}"; ai="${ai:-0}"
        bi="${b[$i]:-0}"; bi="${bi//[^0-9]/}"; bi="${bi:-0}"
        if (( ai > bi )); then return 0; fi
        if (( ai < bi )); then return 1; fi
    done
    return 1  # iguais
}

# ── Subcomandos ───────────────────────────────────────────────────────────────
cmd_version() {
    local current latest
    current=$(current_version)
    local display="$current"
    [[ "$current" == "0.0.0" ]] && display="desconhecida"
    info "Versão atual: $display"
    info "Consultando GitHub por novas versões..."
    latest=$(latest_version) || { warn "Não foi possível consultar o GitHub."; exit 0; }
    info "Versão mais nova: $latest"
    if semver_gt "$latest" "$current"; then
        info "→ Atualização disponível! Execute: rep-ctl update"
    else
        info "→ Você está na versão mais recente."
    fi
}

cmd_check() {
    local current latest
    current=$(current_version)
    latest=$(latest_version) || error "Falha ao consultar GitHub"
    if semver_gt "$latest" "$current"; then
        echo "$latest"
        exit 0
    fi
    exit 1
}

cmd_update() {
    local force="${1:-}"
    local current latest

    current=$(current_version)
    info "Versão atual: $current"
    info "Consultando GitHub por novas versões..."

    local release_json
    release_json=$(fetch_release_json) || error "Falha ao consultar GitHub"

    latest=$(echo "$release_json" | grep -o '"tag_name": *"[^"]*"' \
        | grep -oP 'v?\K\d+\.\d+\.\d+' | head -1)

    if [[ -z "$latest" ]]; then
        error "Não foi possível determinar a versão mais recente."
    fi

    if [[ "$force" != "--force" ]] && ! semver_gt "$latest" "$current"; then
        success "Já na versão mais recente ($current). Nada a fazer."
        exit 0
    fi

    info "Nova versão disponível: $latest (atual: $current)"

    # Baixa binário
    local bin_url
    bin_url=$(echo "$release_json" \
        | grep -o '"browser_download_url": *"[^"]*'"$BINARY_SRC"'[^"]*"' \
        | grep -o 'https://[^"]*' | head -1)
    [[ -z "$bin_url" ]] && error "Binário '$BINARY_SRC' não encontrado no release."

    info "Baixando binário..."
    local tmp_bin
    tmp_bin=$(mktemp /tmp/rep-server-XXXXXX)
    trap 'rm -f "$tmp_bin"' EXIT

    curl -fsSL --max-time 60 -o "$tmp_bin" "$bin_url"
    chmod +x "$tmp_bin"

    # Baixa UI (se disponível)
    local dist_url
    dist_url=$(echo "$release_json" \
        | grep -o '"browser_download_url": *"[^"]*dist\.tar\.gz[^"]*"' \
        | grep -o 'https://[^"]*' | head -1)

    local tmp_dist=""
    if [[ -n "$dist_url" ]]; then
        info "Baixando UI..."
        tmp_dist=$(mktemp /tmp/rep-dist-XXXXXX.tar.gz)
        curl -fsSL --max-time 60 -o "$tmp_dist" "$dist_url"
    fi

    # Aplica binário (o processo é substituído com o serviço reinicializando)
    info "Aplicando atualização..."
    mv "$tmp_bin" "$BINARY_DEST"
    chmod +x "$BINARY_DEST"

    # Aplica UI
    if [[ -n "$tmp_dist" ]]; then
        mkdir -p "$WEB_DEST"
        rm -rf "${WEB_DEST:?}"/*
        tar -xzf "$tmp_dist" -C "$WEB_DEST"
        rm -f "$tmp_dist"
    fi

    # Reinicia serviço
    info "Reiniciando $SERVICE_NAME..."
    if command -v systemctl &>/dev/null; then
        if [[ $EUID -eq 0 ]]; then
            systemctl restart "$SERVICE_NAME"
        else
            sudo systemctl restart "$SERVICE_NAME"
        fi
        success "Serviço reiniciado."
    else
        warn "systemctl não encontrado. Reinicie o serviço manualmente."
    fi

    success "Atualizado para v$latest com sucesso!"
}

# ── Main ──────────────────────────────────────────────────────────────────────
CMD="${1:-help}"
shift || true

case "$CMD" in
    version)  cmd_version ;;
    check)    cmd_check ;;
    update)   cmd_update "${1:-}" ;;
    help|--help|-h)
        echo "Uso: rep-ctl <comando>"
        echo ""
        echo "Comandos:"
        echo "  version    Exibe versão atual e verifica se há atualização"
        echo "  check      Verifica silenciosamente (exit 0 = tem atualização)"
        echo "  update     Aplica atualização e reinicia o serviço"
        echo "  update --force  Força reinstalação mesmo na versão atual"
        ;;
    *)
        echo "Comando desconhecido: $CMD. Use 'rep-ctl help'." >&2
        exit 1
        ;;
esac
