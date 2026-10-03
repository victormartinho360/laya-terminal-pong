# Laya Terminal Pong

Pong de terminal em Rust onde as raquetes adversárias são controladas pelo
[Laya](https://huggingface.co/convaiinnovations/laya) — um modelo de decisão
System 1 não-autoregressivo (ModernBERT/mmBERT + cabeça de decisão), servido
localmente via HTTP (`POST /v1/systemone`).

```
⚡ RUST PONG [🤖 LAYA-1 vs 🤖 LAYA-2] │ LAYA AGENT 1: 03 vs 02 LAYA AGENT 2
```

## Como funciona

- **`src/`** — o jogo (Rust, ratatui + tokio). A cada observação do campo, os
  workers assíncronos enviam o estado (posição/velocidade da bola, previsão de
  ponto de impacto com reflexão nas paredes, posição das raquetes) ao Laya,
  que responde com uma pergunta tipada `choice`: `up1/up2/up3/down1/down2/down3/stay`
  (direção + magnitude). Histerese por margem de decisão elimina jitter.
- **`laya-server/`** — wrapper FastAPI sobre o pacote oficial `laya`.
  Otimizado para CPU: um único checkpoint (`multilingual`, 322M) com encoder
  quantizado em int8 → ~130–150ms por inferência sem GPU, sem cair em swap.
  Em GPU (`LAYA_DEVICE=cuda`), ~40–70ms esperado em uma RTX 2060.
- **Telemetria** — o jogo grava `pong_laya_telemetry.csv` (latência, ação,
  confiança, modelo) e mostra p50/p95/p99 ao vivo no painel lateral.

## Makefile

Os comandos do dia a dia estão encapsulados num `Makefile` (rode `make help`):

| Alvo | O que faz |
|---|---|
| `make build` | Compila o jogo em release |
| `make run` | Compila e roda o jogo (servidor precisa estar no ar) |
| `make up` | Sobe o servidor via Docker em **CPU** (network host, zero overhead) |
| `make nvidia-cuda-dev` | Sobe o servidor via Docker em **CUDA** (compose base + `compose.cuda.yml`; requer [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html)) |
| `make server` | Sobe o servidor **nativo** (sem Docker, via `uv`) em CPU |
| `make server-gpu` | Mesmo, com `LAYA_DEVICE=cuda` |
| `make bench N=100 C=2` | Benchmark de latência do servidor (`N` requisições, `C` em paralelo) |
| `make logs` | Segue os logs do servidor no compose |
| `make down` / `make clean` | Derruba o compose / limpa artefatos do Rust |

O modo CUDA usa o override [`compose.cuda.yml`](compose.cuda.yml), que liga
`LAYA_DEVICE=cuda`, desliga a quantização int8 (ela é CPU-only) e reserva a
GPU via `deploy.resources` — o compose base continua CPU-only e válido sozinho.

## Rodando com Docker (servidor) + jogo nativo

```bash
# 1. servidor de inferência (CPU; para GPU veja compose.yml)
docker compose up --build

# 2. jogo (precisa de um terminal real)
cargo run --release           # ou: ./target/release/terminal_pong_rust
```

O joga contra a IA no modo padrão; `[M]`/`[Tab]` ativa LAYA vs LAYA.

## Rodando tudo nativo

```bash
# terminal 1 — precisa do toolchain Rust (na distro atômica: toolbox)
cd laya-server && uv sync && uv run uvicorn main:app --port 8000

# terminal 2
cargo run --release
```

## Configuração (variáveis de ambiente)

| Var | Default | Descrição |
|---|---|---|
| `LAYA_SERVER_URL` *(jogo)* | `http://127.0.0.1:8000/v1/systemone` | endpoint do servidor |
| `LAYA_TIMEOUT_MS` *(jogo)* | `5000` | timeout por inferência |
| `LAYA_TELEMETRY_PATH` *(jogo)* | `pong_laya_telemetry.csv` | CSV de telemetria |
| `LAYA_DEVICE` *(server)* | `cpu` | `cpu` ou `cuda` |
| `LAYA_MODEL` *(server)* | `multilingual` | `multilingual` / `english` / `typed-decisions` |
| `LAYA_INT8` *(server)* | `1` | quantização int8 do encoder (só CPU) |

## Benchmark do servidor

```bash
uv run scripts/bench_latency.py                # 30 req sequenciais
uv run scripts/bench_latency.py -n 100 -c 2    # simula os dois agentes
```

## Controles

`W`/`↑` e `S`/`↓` movem a raquete · `Espaço` pausa · `R` reseta placar ·
  `M`/`Tab` alterna Humano-vs-IA ↔ IA-vs-IA · `Q`/`Esc` sai.
