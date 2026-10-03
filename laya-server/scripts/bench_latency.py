#!/usr/bin/env python3
"""Benchmark de latência do laya-server (POST /v1/systemone).

Mede tempos de resposta com o payload real que o Terminal Pong envia,
reporta min/p50/p95/p99/max, média, taxa de erro e requisições/segundo.

Uso:
    uv run scripts/bench_latency.py                      # 30 req, 1 worker
    uv run scripts/bench_latency.py -n 100 -c 2          # simula 2 agentes IA
    uv run scripts/bench_latency.py --url http://127.0.0.1:8000 --warmup 5
    uv run scripts/bench_latency.py --csv resultado.csv  # salva latências
"""

import argparse
import csv
import statistics
import sys
import time
from concurrent.futures import ThreadPoolExecutor

import httpx

# Payload no formato exato que o cliente Rust envia (ver src/ai/client.rs).
PONG_PAYLOAD = {
    "state": {
        "game": "pong",
        "court": {"width": 70, "height": 24, "note": "y=0 is the FLOOR, higher y is UP"},
        "ball": {"x": 45.2, "y": 13.8, "velocity_x": 0.94, "velocity_y": 0.34, "speed": 34.2},
        "my_paddle": {"side": "right", "y_bottom": 9.8, "y_center": 12.1, "height": 4.5},
        "opponent_paddle": {"y_center": 10.3},
        "trajectory": "ball approaching right side, impact in ~0.65s",
        "impact_prediction": "predicted ball y at my paddle plane is 15.4",
    },
    "questions": {
        "move": {
            "type": "choice",
            "instructions": (
                "You control the RIGHT paddle of a Pong match. Choose the move that best "
                "brings MY paddle center onto the predicted ball y at my paddle plane. "
                "Higher y means UP. Pick the step size by distance: 1 step for small gaps, "
                "2 for medium, 3 for large gaps."
            ),
            "criteria": {
                "up3": "move UP 3 units: predicted ball y is far ABOVE my paddle center",
                "up2": "move UP 2 units: predicted ball y is moderately ABOVE my paddle center",
                "up1": "move UP 1 unit: predicted ball y is slightly ABOVE my paddle center",
                "stay": "hold position: paddle center already aligned with predicted ball y",
                "down1": "move DOWN 1 unit: predicted ball y is slightly BELOW my paddle center",
                "down2": "move DOWN 2 units: predicted ball y is moderately BELOW my paddle center",
                "down3": "move DOWN 3 units: predicted ball y is far BELOW my paddle center",
            },
        }
    },
}


def percentile(sorted_vals, p):
    if not sorted_vals:
        return 0.0
    idx = max(0, min(len(sorted_vals) - 1, int(len(sorted_vals) * p + 0.999999) - 1))
    return sorted_vals[idx]


def one_request(client, url):
    t0 = time.perf_counter()
    try:
        resp = client.post(url, json=PONG_PAYLOAD)
        elapsed = (time.perf_counter() - t0) * 1000.0
        if resp.status_code != 200:
            return elapsed, f"HTTP {resp.status_code}"
        data = resp.json()
        choice = data.get("answers", {}).get("move", {}).get("choice", "?")
        return elapsed, choice
    except Exception as exc:  # noqa: BLE001
        return (time.perf_counter() - t0) * 1000.0, f"erro: {exc}"


def main():
    ap = argparse.ArgumentParser(description="Benchmark de latência do laya-server")
    ap.add_argument("--url", default="http://127.0.0.1:8000", help="base URL do servidor")
    ap.add_argument("-n", "--requests", type=int, default=30, help="nº de requisições medidas")
    ap.add_argument("--warmup", type=int, default=3, help="requisições de aquecimento (não medidas)")
    ap.add_argument("-c", "--concurrency", type=int, default=1,
                    help="requisições em paralelo (2 simula os dois agentes do Pong)")
    ap.add_argument("--timeout", type=float, default=15.0, help="timeout por requisição (s)")
    ap.add_argument("--csv", metavar="ARQUIVO", help="salva todas as latências em CSV")
    ap.add_argument("-v", "--verbose", action="store_true", help="imprime cada requisição")
    args = ap.parse_args()

    endpoint = f"{args.url.rstrip('/')}/v1/systemone"
    print(f"== laya-server bench == {endpoint} | {args.requests} req | concorrência {args.concurrency}")

    with httpx.Client(timeout=args.timeout) as client:
        # Health check
        try:
            health = client.get(f"{args.url.rstrip('/')}/health", timeout=5).json()
            print(f"servidor: {health}")
        except Exception as exc:  # noqa: BLE001
            print(f"ERRO: servidor não responde em {args.url} ({exc})")
            sys.exit(1)

        # Warmup (cache de tokenizer, JIT do torch, etc.)
        print(f"aquecendo ({args.warmup} req)...", end=" ", flush=True)
        for i in range(args.warmup):
            ms, result = one_request(client, endpoint)
            print(f"warmup{i+1}={ms:.0f}ms[{result}]", end=" ", flush=True)
        print()

        # Medição
        results = []  # (latency_ms, outcome)
        wall_t0 = time.perf_counter()
        with ThreadPoolExecutor(max_workers=args.concurrency) as pool:
            futures = [pool.submit(one_request, client, endpoint) for _ in range(args.requests)]
            for i, fut in enumerate(futures):
                ms, outcome = fut.result()
                results.append((ms, outcome))
                if args.verbose:
                    print(f"  [{i+1:>3}] {ms:8.1f} ms  -> {outcome}")
        wall_s = time.perf_counter() - wall_t0

    lat = sorted(ms for ms, _ in results)
    errors = [(ms, out) for ms, out in results if not out.startswith(("up", "down", "stay"))]

    print()
    print(f"requisições : {len(results)} em {wall_s:.1f}s ({len(results)/wall_s:.2f} req/s)")
    print(f"erros       : {len(errors)}")
    for ms, out in errors[:5]:
        print(f"  - {ms:.0f}ms {out}")
    print(f"min         : {lat[0]:.0f} ms")
    print(f"média       : {statistics.mean(lat):.0f} ms")
    print(f"p50         : {percentile(lat, 0.50):.0f} ms")
    print(f"p95         : {percentile(lat, 0.95):.0f} ms")
    print(f"p99         : {percentile(lat, 0.99):.0f} ms")
    print(f"max         : {lat[-1]:.0f} ms")
    print(f"desvio      : {statistics.pstdev(lat):.0f} ms")

    if args.csv:
        with open(args.csv, "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["seq", "latency_ms", "outcome"])
            for i, (ms, out) in enumerate(results, 1):
                w.writerow([i, f"{ms:.1f}", out])
        print(f"CSV salvo em: {args.csv}")


if __name__ == "__main__":
    main()
