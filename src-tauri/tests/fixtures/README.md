# Miner output fixtures

All captured 2026-08-13 from binaries in ~/src/dinero-v8/build
(version: dinero-miner 8.0.0, commit dce2c5da3bbe).

- dinero-miner.log — solo CPU on regtest (--rpcport 20996 --threads 2 --force,
  90 s): 131 real BLOCK FOUND box sequences + "Block ACCEPTED by daemon!" lines.
  Regtest finds blocks instantly, so hashrate lines legitimately read 0.00 MH/s.
- gpu-miner.log — solo GPU (metal) on regtest (--backend metal, 45 s): real
  26.8 MH/s hashrate lines ("GPU: ... MH/s | Total: ... | Blocks: N") and the
  same block box (adds Height:/Nonce:/Hash: inside the box).
- dinero-stratum-worker.log — live capture against 173.249.200.59:3333, which
  was DOWN at capture time: real connect/retry/failure lines + periodic stat
  lines. The section marked "source-derived samples" reproduces the
  success-path lines verbatim from tools/dinero_stratum_worker.cpp
  (line refs in the marker) because no live pool was reachable — replace with
  a live capture when the pool is redeployed.
