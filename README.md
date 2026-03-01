# Ryanne REP — Agente de Ponto IDClass

Agente de sincronização de registros de ponto para relógios **IDClass** (AFD), integrado ao sistema Ryanne. Roda como serviço headless em container LXC no Proxmox VE e expõe um painel web para monitoramento e configuração.

---

## Como funciona

```
Relógio IDClass ──► rep-server (LXC) ──► Ryanne Sistema (Next.js)
                         │
                    Painel Web
                  http://IP:3001
```

1. O agente lê o AFD do relógio via API IDClass
2. Filtra registros por colaboradores ativos no sistema
3. Envia lotes para `POST /api/punch-collector` com deduplicação por NSR
4. Reporta `lastSyncAt` / `lastError` de volta para o cadastro do relógio no sistema

---

## Instalação (Proxmox VE)

**Uma linha no terminal do Proxmox:**

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/oswaldocavalcante/rep/main/deploy/install-lxc.sh)
```

O script cria automaticamente um container Debian 12, baixa o binário e a UI do GitHub Release mais recente, configura o serviço systemd e exibe o IP e a URL do painel ao final.

### Parâmetros opcionais

| Parâmetro | Default | Descrição |
|---|---|---|
| `--app-url` | — | URL do sistema Ryanne (pré-configura sem abrir a UI) |
| `--api-key` | — | Chave de API punch-collector |
| `--clock-id` | — | UUID do relógio cadastrado no sistema |
| `--port` | `3001` | Porta do painel web |
| `--ctid` | `200` | ID do container Proxmox |
| `--hostname` | `rep-ponto` | Hostname do container |
| `--memory` | `256` | RAM em MB |
| `--storage` | `local-lvm` | Storage Proxmox |
| `--bridge` | `vmbr0` | Bridge de rede |

**Exemplo com pré-configuração completa:**
```bash
bash <(curl -fsSL https://raw.githubusercontent.com/oswaldocavalcante/rep/main/deploy/install-lxc.sh) \
  --app-url  https://sistema.ryanne.com.br \
  --api-key  SUA_CHAVE \
  --clock-id UUID-DO-RELOGIO
```

Após a instalação, acesse o painel em **`http://IP-DO-CT:3001`** com a senha padrão `admin`.

---

## API HTTP

O `rep-server` expõe uma API REST na porta `3001`.

### Rotas públicas

| Método | Rota | Descrição |
|---|---|---|
| `GET` | `/health` | Health check |
| `POST` | `/auth/login` | Login (`{ password }` → `{ token }`) |

### Rotas protegidas (Bearer token)

| Método | Rota | Descrição |
|---|---|---|
| `GET` | `/auth/me` | Verifica sessão |
| `POST` | `/auth/logout` | Revoga token |
| `PUT` | `/api/auth/password` | Troca senha |
| `GET` | `/api/status` | Status da última sincronização |
| `GET` | `/api/config` | Lê configuração atual |
| `PUT` | `/api/config` | Salva configuração |
| `POST` | `/api/provision` | Provisiona agente via `{ app_url, api_key, clock_id }` |
| `POST` | `/api/test-connection` | Testa conexão com o relógio |
| `POST` | `/api/sync/run` | Executa sincronização manualmente |
| `POST` | `/api/sync/reset` | Reseta cursor de sincronização (NSR=0) |
| `POST` | `/api/sync/reprocess` | Reseta e sincroniza imediatamente |
| `GET` | `/api/logs` | Lista histórico de sincronizações |

---

## Integração com o sistema Ryanne

O agente consome dois endpoints do sistema:

| Endpoint | Uso |
|---|---|
| `GET /api/time-clocks/:id/config` | Busca IP, usuário e senha do relógio no provisionamento |
| `POST /api/time-clocks/:id/sync-status` | Reporta `lastSyncAt` e `lastError` após cada ciclo |

A chave de API é a mesma configurada em **Configurações → Chave API Punch Collector** no sistema.

---

## Variáveis de ambiente

Configuradas em `/etc/rep/env` no container:

| Variável | Default | Descrição |
|---|---|---|
| `REP_PORT` | `3001` | Porta do servidor HTTP |
| `REP_WEB_DIR` | `/usr/share/rep/web` | Diretório da UI estática |
| `REP_APP_URL` | — | URL do sistema (provisionamento automático) |
| `REP_API_KEY` | — | Chave de API (provisionamento automático) |
| `REP_CLOCK_ID` | — | ID do relógio (provisionamento automático) |
| `RUST_LOG` | `info` | Nível de log (`debug`, `info`, `warn`, `error`) |

---

## Desenvolvimento local

### Pré-requisitos

- [Rust](https://rustup.rs/) stable
- [Node.js](https://nodejs.org/) 20+
- [Tauri CLI v2](https://tauri.app) (para modo desktop)

### Rodar a UI em modo dev (sem Tauri)

```bash
# Terminal 1 — inicia o rep-server local
cd src-tauri
cargo run --bin rep-server

# Terminal 2 — inicia o Vite com proxy para :3001
npm run dev
```

> O `src/lib/api.ts` detecta automaticamente que a porta não é `3001` e redireciona chamadas para `localhost:3001`.

### Rodar como app desktop (Tauri)

```bash
npm run tauri dev
```

### Build de produção

```bash
# Binário Linux x86_64 (para deploy LXC)
cd src-tauri
cargo build --release --bin rep-server

# UI estática
cd ..
npm run build
# Artefato em dist/
```

---

## Release e CI

Qualquer tag `v*` disparada no GitHub executa o workflow `.github/workflows/release.yml` que:

1. Compila `rep-server` como binário estático Linux (musl)
2. Faz o build da UI React
3. Publica um GitHub Release com os artefatos: `rep-server-linux-x86_64`, `dist.tar.gz`, `install-lxc.sh`, `rep-server.service`

```bash
git tag v1.0.0
git push --tags
```

---

## Estrutura do projeto

```
rep/
├── src/                        # UI React (Vite)
│   ├── lib/api.ts              # Cliente HTTP centralizado
│   └── pages/
│       ├── Login.tsx
│       ├── Status.tsx
│       ├── Config.tsx
│       └── Logs.tsx
├── src-tauri/                  # Rust
│   └── src/
│       ├── lib.rs
│       ├── main.rs             # Entry point Tauri (desktop)
│       ├── bin/rep-server.rs   # Entry point headless (LXC)
│       ├── auth.rs             # TokenStore + hash de senha
│       ├── server.rs           # Router axum + handlers
│       ├── sync.rs             # Loop de sincronização + provisionamento
│       ├── collector.rs        # Envio para punch-collector
│       ├── idclass.rs          # Leitura de AFD (IDClass)
│       ├── config.rs           # Leitura/escrita de config.toml
│       └── state.rs            # Persistência de estado e logs
└── deploy/
    ├── install-lxc.sh          # Instalador one-click Proxmox
    └── rep-server.service      # Unit systemd
```

---

## Operação

```bash
# Ver logs do serviço
pct exec <CTID> -- journalctl -u rep-server -f

# Reiniciar serviço
pct exec <CTID> -- systemctl restart rep-server

# Editar variáveis de ambiente
pct exec <CTID> -- nano /etc/rep/env
pct exec <CTID> -- systemctl restart rep-server
```
