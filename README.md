# DineroMiner

A minimal cross-platform GUI miner for the Dinero network. Paste any valid
Dinero address — from any wallet (iOS, macOS, desktop, paper) — pick CPU or
GPU, press Start, and mine to that address. No wallet, no keys, and no chain
state on the mining machine.

Thin Tauri shell over the proven miner binaries from
[dinero-v8](https://github.com/DineroLabs/dinero-v8)
(`dinero-stratum-worker`, `dinero-miner`, `dinero-gpu-miner`), run as
supervised sidecar processes. The GUI never implements hashing, consensus, or
mining protocols itself.

**Status:** design phase. See
[`docs/superpowers/specs/2026-08-13-miner-gui-design.md`](docs/superpowers/specs/2026-08-13-miner-gui-design.md).

| Mode | CPU | GPU (Metal / CUDA / OpenCL) |
|------|-----|------------------------------|
| Pool | ✅ | ⏳ needs upstream stratum support in the GPU miner |
| Solo (node RPC) | ✅ | ✅ |

Platforms: macOS (arm64), Windows (x64), Linux (x64).
