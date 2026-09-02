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
CTL_DEST="/usr/local/bin/rep-ctl"
WEB_DEST="/usr/share/rep/web"
SERVICE_NAME="rep-server"
SERVICE_DROPIN="/etc/systemd/system/rep-server.service.d/10-rep-ctl.conf"
SUDOERS_FILE="/etc/sudoers.d/rep-ctl"
ENV_FILE="/etc/rep/env"
GH_API="https://api.github.com/repos/${REPO}/releases/latest"

# Setado por migrate_config() quando algo em disco muda (exige daemon-reload + restart)
MIGRATED=0

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

restart_service() {
    if ! command -v systemctl &>/dev/null; then
        warn "systemctl não encontrado. Reinicie o serviço manualmente."
        return
    fi
    if [[ $EUID -eq 0 ]]; then
        systemctl restart "$SERVICE_NAME"
    else
        sudo systemctl restart "$SERVICE_NAME"
    fi
    success "Serviço reiniciado."
}

# ── Migrações de configuração (idempotentes) ─────────────────────────────────
# Ajusta o que o install-lxc.sh passou a gerar mas que updates antigos não
# aplicam: painel na porta 80, capability p/ bind não-root e regra de sudo.
# Só roda como root (via rep-update.service); sob a UI (usuário 'rep') é no-op.
migrate_config() {
    [[ $EUID -ne 0 ]] && return 0
    command -v systemctl &>/dev/null || return 0

    # 1. Drop-in systemd: bind na porta 80 + desabilita no-new-privileges (sudo)
    local amb nnp
    amb=$(systemctl show "$SERVICE_NAME" -p AmbientCapabilities --value 2>/dev/null || true)
    nnp=$(systemctl show "$SERVICE_NAME" -p NoNewPrivileges --value 2>/dev/null || true)
    if [[ "$amb" != *cap_net_bind_service* || "$nnp" == "yes" ]]; then
        if [[ ! -f "$SERVICE_DROPIN" ]]; then
            mkdir -p "$(dirname "$SERVICE_DROPIN")"
            cat > "$SERVICE_DROPIN" <<'EOF'
# Gerado por rep-ctl — não editar manualmente
[Service]
AmbientCapabilities=CAP_NET_BIND_SERVICE
NoNewPrivileges=false
EOF
            info "Drop-in systemd instalado (porta 80 + auto-update pela UI)."
            MIGRATED=1
        fi
    fi

    # 2. sudoers: installs antigos só permitiam 'restart rep-server' e apontavam
    #    para /bin/systemctl; o botão da UI agora dispara o rep-update.service.
    if command -v visudo &>/dev/null; then
        if [[ ! -f "$SUDOERS_FILE" ]] || ! grep -qF 'start --no-block rep-update.service' "$SUDOERS_FILE"; then
            printf 'rep ALL=(ALL) NOPASSWD: /usr/bin/systemctl start --no-block rep-update.service, /bin/systemctl start --no-block rep-update.service, /usr/bin/systemctl restart rep-server, /bin/systemctl restart rep-server\n' \
                > "${SUDOERS_FILE}.new"
            chmod 440 "${SUDOERS_FILE}.new"
            if visudo -cqf "${SUDOERS_FILE}.new"; then
                mv "${SUDOERS_FILE}.new" "$SUDOERS_FILE"
                info "Regra sudoers do rep-ctl atualizada."
                MIGRATED=1
            else
                rm -f "${SUDOERS_FILE}.new"
                warn "sudoers gerado é inválido; regra mantida como está."
            fi
        fi
    fi

    # 3. REP_PORT: default antigo (3001) ou ausente → 80. Portas customizadas ficam.
    if [[ -f "$ENV_FILE" ]]; then
        if ! grep -q '^REP_PORT=' "$ENV_FILE"; then
            echo 'REP_PORT=80' >> "$ENV_FILE"
            info "REP_PORT ausente em $ENV_FILE — definido como 80."
            MIGRATED=1
        elif grep -qE '^REP_PORT=3001[[:space:]]*$' "$ENV_FILE"; then
            sed -i -E 's/^REP_PORT=3001[[:space:]]*$/REP_PORT=80/' "$ENV_FILE"
            info "REP_PORT migrado de 3001 para 80 — painel agora em http://IP (sem porta)."
            MIGRATED=1
        fi
    fi

    if [[ "$MIGRATED" -eq 1 ]]; then
        systemctl daemon-reload
    fi
    return 0
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

    [[ $EUID -ne 0 ]] && error "Execute como root: sudo rep-ctl update (ou use o botão na UI)."

    current=$(current_version)
    info "Versão atual: $current"

    migrate_config

    info "Consultando GitHub por novas versões..."

    local release_json
    release_json=$(fetch_release_json) || error "Falha ao consultar GitHub"

    latest=$(echo "$release_json" | grep -o '"tag_name": *"[^"]*"' \
        | grep -oP 'v?\K\d+\.\d+\.\d+' | head -1)

    if [[ -z "$latest" ]]; then
        error "Não foi possível determinar a versão mais recente."
    fi

    if [[ "$force" != "--force" ]] && ! semver_gt "$latest" "$current"; then
        if [[ "$MIGRATED" -eq 1 ]]; then
            info "Configuração migrada; reiniciando serviço..."
            restart_service
        fi
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

    # Auto-atualiza o próprio rep-ctl (para receber futuras migrações)
    if [[ $EUID -eq 0 ]]; then
        local ctl_url
        ctl_url=$(echo "$release_json" \
            | grep -o '"browser_download_url": *"[^"]*rep-update\.sh[^"]*"' \
            | grep -o 'https://[^"]*' | head -1)
        if [[ -n "$ctl_url" ]] \
            && curl -fsSL --max-time 30 -o "${CTL_DEST}.new" "$ctl_url" \
            && head -1 "${CTL_DEST}.new" | grep -q '^#!'; then
            chmod +x "${CTL_DEST}.new"
            mv "${CTL_DEST}.new" "$CTL_DEST"
            info "rep-ctl atualizado."
        else
            rm -f "${CTL_DEST}.new"
        fi
    fi

    # Reinicia serviço
    info "Reiniciando $SERVICE_NAME..."
    restart_service

    success "Atualizado para v$latest com sucesso!"
}

cmd_migrate() {
    [[ $EUID -ne 0 ]] && error "Execute como root (sudo rep-ctl migrate)."
    migrate_config
    if [[ "$MIGRATED" -eq 1 ]]; then
        info "Reiniciando serviço para aplicar..."
        restart_service
        success "Migração concluída."
    else
        success "Nada a migrar; configuração já está atualizada."
    fi
}

# ── Main ──────────────────────────────────────────────────────────────────────
CMD="${1:-help}"
shift || true

case "$CMD" in
    version)  cmd_version ;;
    check)    cmd_check ;;
    update)   cmd_update "${1:-}" ;;
    migrate)  cmd_migrate ;;
    help|--help|-h)
        echo "Uso: rep-ctl <comando>"
        echo ""
        echo "Comandos:"
        echo "  version    Exibe versão atual e verifica se há atualização"
        echo "  check      Verifica silenciosamente (exit 0 = tem atualização)"
        echo "  update     Aplica atualização e reinicia o serviço"
        echo "  update --force  Força reinstalação mesmo na versão atual"
        echo "  migrate    Aplica migrações de config (porta 80, sudoers) sem atualizar"
        ;;
    *)
        echo "Comando desconhecido: $CMD. Use 'rep-ctl help'." >&2
        exit 1
        ;;
esac
