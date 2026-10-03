"""Servidor de inferência Laya para o Terminal Pong.

Expõe POST /v1/systemone (contrato compatível com TypeSafe Jev, conforme
a documentação oficial: https://huggingface.co/convaiinnovations/laya).

Decisões de performance para CPU (validadas em benchmark nesta máquina,
i5-1235U / 15GB RAM):

1. UM checkpoint residente (default: multilingual, 322M params).
   O Router com preload de 3 checkpoints estoura a RAM e cai em swap,
   elevando a latência de ~170ms para ~2.5s por inferência.
2. Quantização dinâmica int8 do encoder (~150ms e decisões mais nítidas).

Variáveis de ambiente:
    LAYA_MODEL    checkpoint: multilingual (default) | english | typed-decisions
    LAYA_INT8     "1" (default) quantiza o encoder para int8; "0" = fp32
    LAYA_DEVICE   cpu (default) | cuda

Uso:
    uv run uvicorn main:app --host 0.0.0.0 --port 8000
"""

import os
import time
import logging
from typing import Any, Dict

from fastapi import FastAPI, HTTPException
from fastapi.responses import JSONResponse
from pydantic import BaseModel

# USE_TF=0 evita deadlock do abseil quando tensorflow está instalado
os.environ.setdefault("USE_TF", "0")

import laya  # noqa: E402

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
)
log = logging.getLogger("laya-server")

MODEL = os.environ.get("LAYA_MODEL", "multilingual")
DEVICE = os.environ.get("LAYA_DEVICE", "cpu")
INT8 = os.environ.get("LAYA_INT8", "1") == "1"

app = FastAPI(title="Laya Decision Server", version="2.0.0")

log.info("Carregando Laya (model=%s, device=%s, int8=%s)...", MODEL, DEVICE, INT8)
_load_t0 = time.perf_counter()
agent = laya.load("convaiinnovations/laya", subfolder=MODEL, device=DEVICE)

if INT8 and DEVICE == "cpu":
    import torch

    agent.model.encoder = torch.quantization.quantize_dynamic(
        agent.model.encoder, {torch.nn.Linear}, dtype=torch.qint8
    )
    log.info("Encoder quantizado para int8.")

log.info("Modelo pronto em %.2fs", time.perf_counter() - _load_t0)


class SystemOneRequest(BaseModel):
    """Mesmo shape do POST /v1/systemone do laya-serve / TypeSafe Jev."""

    state: Any
    questions: Dict[str, Any]

    class Config:
        extra = "ignore"  # ignora campos desconhecidos, como o laya-serve


@app.get("/health")
def health() -> Dict[str, Any]:
    return {
        "status": "ok",
        "model": f"convaiinnovations/laya/{MODEL}",
        "device": DEVICE,
        "int8": INT8,
    }


@app.post("/v1/systemone")
def systemone(req: SystemOneRequest):
    started = time.perf_counter()
    try:
        result = agent.predict(req.state, req.questions)
    except ValueError as exc:
        # Pergunta malformada: o laya-serve devolve 422 nomeando o problema
        raise HTTPException(status_code=422, detail=str(exc)) from exc
    except Exception as exc:  # noqa: BLE001
        log.exception("Falha na inferência")
        return JSONResponse(status_code=500, content={"error": str(exc)})

    # Mantém o campo `routing` do contrato do Router para os clientes.
    result.setdefault("routing", {"model": MODEL, "repo": f"convaiinnovations/laya/{MODEL}"})

    elapsed_ms = (time.perf_counter() - started) * 1000.0
    log.info("inferência ok: latency=%.1fms", elapsed_ms)
    return result
