#!/usr/bin/env bash
# ==============================================================================
# install-lxc.sh — Instalação do Ryanne REP em container LXC no Proxmox VE
#
# Uso mínimo (download automático do GitHub):
#   bash <(curl -fsSL https://raw.githubusercontent.com/USER/REPO/main/deploy/install-lxc.sh) \
#     --repo USER/REPO
#
# Com pré-configuração (sem precisar abrir a UI):
#   bash <(curl -fsSL ...) \
#     --repo     USER/REPO \
#     --app-url  https://seu-sistema.com.br \
#     --api-key  SUA_CHAVE_API \
#     --clock-id UUID-DO-RELOGIO
#
# Com arquivos locais (sem acesso ao GitHub):
#   ./install-lxc.sh --binary ./rep-server-linux-x86_64 --web-dir ./dist
# ==============================================================================
set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
REPO="oswaldocavalcante/rep"
GH_TOKEN=""
RELEASE_TAG="latest"
APP_URL=""
API_KEY=""
CLOCK_ID=""
REP_PORT="3001"
CTID="200"
HOSTNAME="rep-ponto"
MEMORY="256"
STORAGE="local-lvm"
BRIDGE="vmbr0"
TEMPLATE_STORAGE="local"
BINARY_PATH=""
WEB_DIR_PATH=""

# ── Parse args ────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)       REPO="$2";          shift 2 ;;
    --token)      GH_TOKEN="$2";      shift 2 ;;
    --release)    RELEASE_TAG="$2";   shift 2 ;;
    --app-url)    APP_URL="$2";       shift 2 ;;
    --api-key)    API_KEY="$2";       shift 2 ;;
    --clock-id)   CLOCK_ID="$2";      shift 2 ;;
    --port)       REP_PORT="$2";      shift 2 ;;
    --ctid)       CTID="$2";          shift 2 ;;
    --hostname)   HOSTNAME="$2";      shift 2 ;;
    --memory)     MEMORY="$2";        shift 2 ;;
    --storage)    STORAGE="$2";       shift 2 ;;
    --bridge)     BRIDGE="$2";        shift 2 ;;
    --binary)     BINARY_PATH="$2";   shift 2 ;;
    --web-dir)    WEB_DIR_PATH="$2";  shift 2 ;;
    *) echo "Parâmetro desconhecido: $1"; exit 1 ;;
  esac
done

error() { echo "ERRO: $*" >&2; exit 1; }

command -v pct  >/dev/null 2>&1 || error "Este script deve ser executado em um host Proxmox VE"
command -v curl >/dev/null 2>&1 || error "curl não encontrado"

# ── Resolve artefatos (GitHub Release ou local) ───────────────────────────────
WORKDIR="$(mktemp -d /tmp/rep-install-XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

if [[ -z "$BINARY_PATH" || -z "$WEB_DIR_PATH" ]]; then
  [[ -z "$REPO" ]] && error "Informe --repo USER/REPO para baixar do GitHub, ou --binary e --web-dir para usar arquivos locais"

  echo "══ Baixando artefatos do GitHub Release (${REPO}@${RELEASE_TAG})..."

  if [[ "$RELEASE_TAG" == "latest" ]]; then
    API_URL="https://api.github.com/repos/${REPO}/releases/latest"
  else
    API_URL="https://api.github.com/repos/${REPO}/releases/tags/${RELEASE_TAG}"
  fi

  RELEASE_JSON=$(curl -fsSL ${GH_TOKEN:+-H "Authorization: token $GH_TOKEN"} "$API_URL") \
    || error "Falha ao acessar a API do GitHub. Verifique --repo e se há um release publicado."

  get_asset_url() {
    echo "$RELEASE_JSON" | grep -o '"browser_download_url": *"[^"]*'"$1"'[^"]*"' | grep -o 'https://[^"]*' | head -1
  }

  BINARY_URL=$(get_asset_url "rep-server-linux-x86_64")
  DIST_URL=$(get_asset_url "dist.tar.gz")
  SERVICE_URL=$(get_asset_url "rep-server.service")

  [[ -z "$BINARY_URL" ]] && error "Binário 'rep-server-linux-x86_64' não encontrado no release."
  [[ -z "$DIST_URL"   ]] && error "UI 'dist.tar.gz' não encontrada no release."

  echo "   Baixando binário..."
  curl -fsSL ${GH_TOKEN:+-H "Authorization: token $GH_TOKEN"} -o "$WORKDIR/rep-server" "$BINARY_URL"
  chmod +x "$WORKDIR/rep-server"

  echo "   Baixando UI..."
  curl -fsSL ${GH_TOKEN:+-H "Authorization: token $GH_TOKEN"} -o "$WORKDIR/dist.tar.gz" "$DIST_URL"
  mkdir -p "$WORKDIR/dist"
  tar -xzf "$WORKDIR/dist.tar.gz" -C "$WORKDIR/dist"

  if [[ -n "$SERVICE_URL" ]]; then
    curl -fsSL ${GH_TOKEN:+-H "Authorization: token $GH_TOKEN"} -o "$WORKDIR/rep-server.service" "$SERVICE_URL"
    SERVICE_PATH="$WORKDIR/rep-server.service"
  else
    SERVICE_PATH=""
  fi

  BINARY_PATH="$WORKDIR/rep-server"
  WEB_DIR_PATH="$WORKDIR/dist"
  echo "   Artefatos prontos."
else
  [[ -f "$BINARY_PATH" ]] || error "Binário não encontrado em '$BINARY_PATH'"
  [[ -d "$WEB_DIR_PATH" ]] || error "Diretório da UI não encontrado em '$WEB_DIR_PATH'"
  SERVICE_PATH="$(cd "$(dirname "$0")" && pwd)/rep-server.service"
fi

[[ -f "$SERVICE_PATH" ]] || error "Arquivo rep-server.service não encontrado"

# ── Verifica template Debian 12 ───────────────────────────────────────────────
echo "══ Verificando template Debian 12 LXC..."
TEMPLATE="debian-12-standard_12.7-1_amd64.tar.zst"
TEMPLATE_PATH="${TEMPLATE_STORAGE}:vztmpl/${TEMPLATE}"

if ! pveam list "${TEMPLATE_STORAGE}" 2>/dev/null | grep -q "debian-12"; then
  echo "   Baixando template Debian 12..."
  pveam update
  pveam download "${TEMPLATE_STORAGE}" "${TEMPLATE}" || \
    error "Falha ao baixar template. Verifique conectividade e storage '${TEMPLATE_STORAGE}'."
fi

echo "   Template encontrado."

# ── Cria o container ──────────────────────────────────────────────────────────
echo "══ Criando container LXC (CT${CTID})..."

if pct status "${CTID}" >/dev/null 2>&1; then
  echo "   CT${CTID} já existe. Parando e destruindo..."
  pct stop "${CTID}" 2>/dev/null || true
  sleep 2
  pct destroy "${CTID}"
fi

pct create "${CTID}" "${TEMPLATE_PATH}" \
  --hostname "${HOSTNAME}" \
  --memory "${MEMORY}" \
  --rootfs "${STORAGE}:2" \
  --net0 "name=eth0,bridge=${BRIDGE},ip=dhcp" \
  --unprivileged 1 \
  --features "nesting=0" \
  --start 1 \
  --ostype debian

echo "   Aguardando inicialização do container..."
sleep 5

# ── Aguarda rede ─────────────────────────────────────────────────────────────
echo "══ Aguardando rede no container..."
for i in $(seq 1 20); do
  if pct exec "${CTID}" -- ping -c1 -W2 8.8.8.8 >/dev/null 2>&1; then
    echo "   Rede OK após ${i}s"
    break
  fi
  sleep 1
done

# ── Instala dependências ──────────────────────────────────────────────────────
echo "══ Instalando pacotes no container..."
pct exec "${CTID}" -- bash -c "
  apt-get update -qq &&
  apt-get install -y --no-install-recommends curl ca-certificates
"

# ── Cria usuário e diretórios ─────────────────────────────────────────────────
echo "══ Criando usuário 'rep' e diretórios..."
pct exec "${CTID}" -- bash -c "
  id rep >/dev/null 2>&1 || useradd -r -s /bin/false -d /var/lib/rep rep
  mkdir -p /var/lib/rep /etc/rep /usr/share/rep/web
  chown -R rep:rep /var/lib/rep /etc/rep
"

# ── Copia binário ─────────────────────────────────────────────────────────────
echo "══ Copiando binário rep-server..."
pct push "${CTID}" "$BINARY_PATH" /usr/local/bin/rep-server
pct exec "${CTID}" -- chmod +x /usr/local/bin/rep-server

# ── Copia UI estática ─────────────────────────────────────────────────────────
echo "══ Copiando UI web..."
pct exec "${CTID}" -- rm -rf /usr/share/rep/web
pct exec "${CTID}" -- mkdir -p /usr/share/rep/web
(cd "$WEB_DIR_PATH" && tar -c .) | pct exec "${CTID}" -- bash -c "tar -x -C /usr/share/rep/web"
echo "   UI copiada."

# ── Cria /etc/rep/env ─────────────────────────────────────────────────────────
echo "══ Criando /etc/rep/env..."
ENV_CONTENT="REP_PORT=${REP_PORT}
REP_WEB_DIR=/usr/share/rep/web
RUST_LOG=info"

[[ -n "$APP_URL"  ]] && ENV_CONTENT+=$'\nREP_APP_URL='"${APP_URL}"
[[ -n "$API_KEY"  ]] && ENV_CONTENT+=$'\nREP_API_KEY='"${API_KEY}"
[[ -n "$CLOCK_ID" ]] && ENV_CONTENT+=$'\nREP_CLOCK_ID='"${CLOCK_ID}"

printf '%s\n' "$ENV_CONTENT" | pct exec "${CTID}" -- bash -c "cat > /etc/rep/env && chmod 600 /etc/rep/env && chown rep:rep /etc/rep/env"

# ── Instala unit systemd ──────────────────────────────────────────────────────
echo "══ Instalando serviço systemd..."
pct push "${CTID}" "$SERVICE_PATH" /etc/systemd/system/rep-server.service
pct exec "${CTID}" -- bash -c "systemctl daemon-reload && systemctl enable rep-server && systemctl start rep-server"

# ── Verificação final ─────────────────────────────────────────────────────────
echo "══ Verificando serviço..."
sleep 3
STATUS=$(pct exec "${CTID}" -- systemctl is-active rep-server 2>/dev/null || echo "unknown")
CT_IP=$(pct exec "${CTID}" -- bash -c "hostname -I 2>/dev/null | awk '{print \$1}'" 2>/dev/null || echo "obtendo...")

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║          Ryanne REP — Instalação concluída           ║"
echo "╠══════════════════════════════════════════════════════╣"
printf "║  Container ID : CT%-34s║\n" "${CTID}"
printf "║  IP           : %-36s║\n" "${CT_IP}"
printf "║  Painel web   : http://%-29s║\n" "${CT_IP}:${REP_PORT}"
printf "║  Serviço      : %-36s║\n" "${STATUS}"
echo "╠══════════════════════════════════════════════════════╣"
echo "║  Senha padrão de acesso ao painel: admin             ║"
echo "║  Recomendado: altere em Configurações → Senha        ║"
echo "╚══════════════════════════════════════════════════════╝"

if [[ "$STATUS" != "active" ]]; then
  echo ""
  echo "AVISO: serviço não está ativo. Verifique:"
  echo "  pct exec ${CTID} -- journalctl -u rep-server -n 30"
  exit 1
fi
