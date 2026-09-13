# DineroMiner

A small cross-platform GUI miner for the Dinero network. Paste a payout
address, pick CPU or GPU, press Start, and mine to that address. No wallet,
no keys, and no chain state on the mining machine. It is a thin
[Tauri 2](https://tauri.app) shell that runs the existing miner binaries
as supervised sidecar processes: `dinero-sv2-miner` / `dinero-sv2-gpu-miner`
from [dinero-sv2](https://github.com/DineroLabs/dinero-sv2) for pool mining,
and `dinero-miner` / `dinero-gpu-miner` from
[dinero-v8](https://github.com/DineroLabs/dinero-v8) for solo mining. The GUI
never implements hashing, consensus, or mining protocols itself.

> ## Status
>
> **v1 is built and working from source; no installer is published yet.**
> There are no GitHub Releases and no CI on this repo. Anything you see here
> has to be compiled on your own machine (macOS only for now, see below).
>
> **To mine today, use the dinero-sv2 installers** (notarized macOS
> arm64/x86_64, Windows x64, Linux x86_64/aarch64; release
> [`miner-v0.2.8`](https://github.com/DineroLabs/dinero-sv2/releases/tag/miner-v0.2.8),
> checksums in `SHA256SUMS`):
>
> ```sh
> # macOS / Linux, CPU
> curl -fsSL https://raw.githubusercontent.com/DineroLabs/dinero-sv2/main/scripts/install.sh | sh
> # macOS / Linux, GPU
> curl -fsSL https://raw.githubusercontent.com/DineroLabs/dinero-sv2/main/scripts/install-gpu.sh | sh
> ```
>
> ```powershell
> # Windows, CPU
> irm https://raw.githubusercontent.com/DineroLabs/dinero-sv2/main/scripts/install.ps1 | iex
> # Windows, GPU
> irm https://raw.githubusercontent.com/DineroLabs/dinero-sv2/main/scripts/install-gpu.ps1 | iex
> ```
>
> The same one-liners are on <https://dinerolabs.org>.

## What works

| Mode | CPU | GPU (Metal / CUDA / OpenCL) |
|------|-----|------------------------------|
| Pool (Stratum V2, `pool.dinerolabs.org:4444`) | yes (`dinero-sv2-miner`) | yes (`dinero-sv2-gpu-miner`) |
| Solo (your own node's RPC) | yes (`dinero-miner`) | yes (`dinero-gpu-miner`) |

- Pool mode defaults to the public SV2 pool and pins its static Noise
  server key, so the app will not talk to an impostor pool. Both can be
  overridden in `settings.json` (`pool_endpoint`, `pool_pubkey`).
- Solo mode needs a reachable `dinerod` RPC URL
  (`http://user:pass@host:20998`) and is under "Advanced".
- Sidecar binaries are hash-checked at startup against `scripts/sidecars.lock`.

## Address requirement

**Pool payouts require a Taproot address (`din1p…`).** The pool pays out via
a P2TR script; the app refuses to start pool mining with any other address
type (`ResolveError::NotTaproot` in `src-tauri/src/worksource.rs`). Any
current Dinero wallet can generate one. Solo mode passes the address to
your own node and accepts whatever that node accepts.

## Build from source (macOS)

Requirements: Rust stable, `cargo tauri` (Tauri CLI 2.x), Xcode command-line
tools, and local builds of
[dinero-v8](https://github.com/DineroLabs/dinero-v8) (`~/src/dinero-v8/build`)
and [dinero-sv2](https://github.com/DineroLabs/dinero-sv2)
(`~/src/dinero-sv2/target/release`). Override those paths with
`DINERO_BUILD_DIR` / `SV2_BUILD_DIR`.

```sh
./scripts/fetch-sidecars.sh   # stage the four miner binaries + write sidecars.lock
cargo tauri dev               # or: cargo tauri build
```

Signing, notarization, and the release checklist are in
[`docs/RELEASING.md`](docs/RELEASING.md). Windows and Linux builds are not
wired yet: `fetch-sidecars.sh` only knows how to source the macOS binaries.

## Roadmap to a downloadable installer

In order, none started as of 2026-09-13:

1. CI release workflow (GitHub Actions) that builds the Tauri bundle on tag.
2. Pull sidecars from the published dinero-sv2 and dinero-v8 release
   artifacts instead of a local build directory, and regenerate
   `sidecars.lock` from the signed binaries.
3. Developer ID signing + notarization of the macOS DMG in CI (the manual
   flow already exists in `docs/RELEASING.md`).
4. Windows (MSI/NSIS) and Linux (AppImage/deb) bundles, including CUDA/OpenCL
   GPU sidecars for those platforms.
5. First tagged GitHub Release with SHA-256 sums, at which point the Status
   box above goes away.

## Related

- [dinero-v8](https://github.com/DineroLabs/dinero-v8): the node, Qt wallet,
  and solo miners.
- [dinero-sv2](https://github.com/DineroLabs/dinero-sv2): the SV2 pool and
  the CPU/GPU pool miners used as sidecars here.
- Design notes: [`docs/superpowers/specs/2026-08-13-miner-gui-design.md`](docs/superpowers/specs/2026-08-13-miner-gui-design.md).
