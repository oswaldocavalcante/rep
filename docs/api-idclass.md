# API IDClass - Discovery

## Dispositivo
- **Modelo**: iDClass Mult S
- **IP**: 192.168.1.3
- **Firmware**: (obter via get_about)
- **Porta**: 443 (HTTPS)

## Autenticação

### Login
```http
POST /login.fcgi
Content-Type: application/json

{
  "login": "admin",
  "password": "twk192joc"
}
```

**Response:**
```json
{
  "session": "2DxkCmy4Th5aOPJeGGTdbtBG"
}
```

### Verificar sessão
```http
POST /session_is_valid.fcgi?session={session}
Content-Type: application/json

{}
```

### Logout
```http
POST /logout.fcgi?session={session}
Content-Type: application/json

{}
```

## Obter Registros de Ponto (AFD)

### get_afd
Obtém o arquivo AFD (Arquivo Digital de Ponto) com todas as marcações.

```http
POST /get_afd.fcgi?session={session}
Content-Type: application/json

{
  "initial_nsr": 23700
}
```

Ou por data:
```http
POST /get_afd.fcgi?session={session}
Content-Type: application/json

{
  "initial_date": {
    "day": 1,
    "month": 1,
    "year": 2024
  }
}
```

**Response (formato texto):**
```
0000237003270220261300012412825396605b
00002370132702202613000203651979256790

AFD00014003750092011.txt
```

### Formato do AFD (Portaria 1510/2009)

Cada linha tem 32 caracteresfixos:
- **NSR** (9): Número sequencial do registro
- **Data** (8): DDMMAAAA
- **Hora** (6): HHMMSS
- **Código** (5): Código do funcionário/pis
- **Tipo Registro** (1): 1=Entrada, 2=Saída, 3=Entrada intermediária, 4=Saída intermediária
- **PIS** (12): PIS do funcionário
- **Nome** (52): Nome do funcionário (com espaços)
- **Matrícula** (20)
- **Data ponto** (8): AAAAMMDD
- **CRC** (4): Verificação

## Obter Informações do Sistema

### get_system_information
Retorna informações e estatísticas do REP.

```http
POST /get_system_information.fcgi?session={session}
Content-Type: application/json

{}
```

**Response:**
```json
{
  "user_count": 23,
  "template_count": 52,
  "uptime": 53326,
  "ticks": 1110,
  "cuts": 23271,
  "coil_paper": 0,
  "total_paper": 10706,
  "paper_ok": true,
  "low_paper": null,
  "memory": 22,
  "used_mrp": 0,
  "last_nsr": 23701
}
```

### get_about
Retorna informações do REP.

```http
POST /get_about.fcgi?session={session}
Content-Type: application/json

{}
```

## Observações

- O dispositivo usa certificado SSL auto-assinado (ignorar com `danger_accept_invalid_certs`)
- O endpoint `get_afd.fcgi` retorna registros em formato texto, não JSON
- O `last_nsr` indica o número do último registro para evitar duplicatas
- A sessão expira após um tempo (verificar com `session_is_valid`)
