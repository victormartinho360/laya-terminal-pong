# laya-terminal-pong

COMPOSE      := docker compose
COMPOSE_BASE := -f compose.yml
COMPOSE_CUDA := -f compose.yml -f compose.cuda.yml
BINARY       := target/release/laya-terminal-pong

.PHONY: help build run server server-gpu up up-gpu nvidia-cuda-dev down bench logs clean

help: ## Mostra esta ajuda
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | \
	  awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

# ---------- jogo (nativo, precisa do toolchain Rust / toolbox) ----------

build: ## Compila o jogo em release
	cargo build --release

run: build ## Compila e roda o jogo (servidor precisa estar no ar)
	./$(BINARY)

# ---------- servidor nativo (sem docker, via uv) ----------

server: ## Sobe o laya-server nativo em CPU
	cd laya-server && uv run uvicorn main:app --host 0.0.0.0 --port 8000

server-gpu: ## Sobe o laya-server nativo em CUDA
	cd laya-server && LAYA_DEVICE=cuda LAYA_INT8=0 uv run uvicorn main:app --host 0.0.0.0 --port 8000

# ---------- docker compose ----------

up: ## Docker: sobe o servidor em CPU (network host, zero overhead)
	$(COMPOSE) $(COMPOSE_BASE) up --build

nvidia-cuda-dev: ## Docker: sobe o servidor em CUDA (requer NVIDIA Container Toolkit)
	$(COMPOSE) $(COMPOSE_CUDA) up --build

down: ## Derruba o compose
	$(COMPOSE) $(COMPOSE_BASE) down

logs: ## Segue os logs do servidor no compose
	$(COMPOSE) $(COMPOSE_BASE) logs -f laya-server

# ---------- extras ----------

bench: ## Benchmark de latência do servidor (30 req; use N=100 C=2 p/ mais)
	cd laya-server && uv run scripts/bench_latency.py -n $(or $(N),30) -c $(or $(C),1)

clean: ## Remove artefatos de build do Rust
	cargo clean
