# DineroMiner — minimal cross-platform miner GUI (design)

Date: 2026-08-13
Status: draft for owner review

## Purpose

A small desktop app where a user pastes any valid Dinero address — from any
wallet (DineroDPI iOS/macOS, dinero-qt, paper) — picks CPU or GPU, presses
Start, and mines to that address on the device. No wallet, no keys, no chain
state on the mining machine.

The app is a thin shell over the existing, proven miner binaries from
`dinero-v8`. It never implements hashing, consensus, or the mining protocols
itself; a GUI bug can therefore never produce a bad share or an invalid block.

## Goals

- One screen. Address → mode → device → Start. Live hashrate and results.
- Cross-platform from day one: macOS (arm64), Windows (x64), Linux (x64).
- Pool mode against the DineroLabs SV2/stratum pool (default endpoint baked
  in, overridable), and solo mode against a node RPC (local node or a remote
  template endpoint), with the coinbase paying the entered address directly.
- Reuse `dinero-stratum-worker`, `dinero-miner`, and `dinero-gpu-miner` as
  bundled sidecar processes, unmodified where possible.

## Non-goals (v1)

- No wallet features: no balances, no history, no keys. The app never learns
  whether the address received anything.
- No charts, no earnings estimation, no telemetry, no auto-update.
- No thermal management beyond what the miners already do; the thread slider
  and GPU backend choice are the user's throttle.
- No mobile version. iOS remains an address source only (Apple prohibits
  on-device mining in App Store apps, Guideline 2.4.2).

## Supported matrix (v1) — grounded in the actual binaries

| Mode | CPU | GPU |
|------|-----|-----|
| Pool | ✅ `dinero-stratum-worker --stratum … --user <address>[.worker] --threads N` | ❌ gap — the GPU miner speaks only node RPC today |
| Solo | ✅ `dinero-miner --rpc <url> --address <addr> --threads N` | ✅ `dinero-gpu-miner --rpc <url> --address <addr> --backend auto\|metal\|cuda\|opencl` |

Pool+GPU requires a stratum backend in `dinero-gpu-miner` (upstream dinero-v8
work, out of scope for this app). The UI greys out that combination with an
explanatory tooltip rather than pretending.

## Architecture (Approach A — thin shell over sidecars)

Tauri application: Rust core, small web-view UI. Four units with hard seams:

1. **AddressValidator** (pure Rust function)
   - Accepts mainnet transparent addresses: bech32/bech32m, HRP `din`,
     checksum-verified, length-sane. Rejects testnet/regtest (`tdin`/`rdin`)
     and shielded (`dins1…`) prefixes with specific error messages — shielded
     addresses cannot receive coinbase/pool payouts here.
   - Input: string. Output: `Ok(NormalizedAddress)` or a typed error the UI
     renders inline. No I/O, fully unit-tested against vectors generated from
     dinero-v8's own address encoder.

2. **WorkSource** (config resolver, not a network layer)
   - The miners own their protocols; this unit only resolves *which sidecar
     with which arguments* a given (mode, device, settings) tuple maps to,
     per the matrix above.
   - Pool defaults: `stratum+tcp://pool.dinerolabs.org:<port>` (exact default
     endpoint copied from the Contribute feature's config at implementation
     time); advanced field overrides. Worker name defaults to the machine's
     hostname (`address.hostname`).
   - Solo: RPC URL field. Two accepted forms: a local node (auto-detected
     via `--datadir`/cookie, the binaries' native mechanism) or a remote URL
     with credentials embedded (`http://user:pass@host:port`). Requirement:
     if the binaries turn out not to honor URL-embedded credentials, the
     credential plumbing is added upstream in the CLIs (small patch), not
     worked around in the shell.
   - Trust note, shown once in the solo-remote UI: a remote template server
     can waste your hashes but cannot redirect a found block's reward — the
     coinbase pays the address you entered.

3. **MinerProcess** (sidecar supervisor)
   - Spawns the chosen binary, captures stdout/stderr line-streams, and
     translates them into one normalized event type:
     `{hashrate, shares_accepted, shares_rejected, blocks_found, uptime,
     status, last_error}`.
   - Per-binary line parsers live behind this interface; they are fixed
     against each binary's real output during implementation and covered by
     unit tests over captured sample logs. If a line doesn't parse, it is
     surfaced verbatim in a collapsible raw-log view — never silently
     dropped.
   - Restart policy: on unexpected exit, exponential backoff (2 s → 60 s
     cap), banner shows "reconnecting"; a crash loop (5 exits in 2 min)
     stops mining and surfaces the last stderr lines. Stop button always
     terminates the child process tree.

4. **UI** (single screen)
   - Address field with paste button and inline ✓/✗ from AddressValidator;
     Start disabled until valid.
   - Mode toggle: Pool (default) / Solo. Solo reveals the RPC URL field.
   - Device picker: CPU (thread slider, default = physical cores − 1) or GPU
     (backend dropdown populated per platform: Metal on macOS, CUDA/OpenCL on
     Windows/Linux; hidden entirely if no GPU backend probes successfully).
     GPU is greyed out in Pool mode (matrix gap) with tooltip.
   - Start/Stop. While running: large hashrate, accepted/rejected shares
     (pool) or blocks found (solo), uptime, connection status dot.
   - Advanced disclosure: pool endpoint override, worker name, raw log view.
   - Settings (address, mode, endpoint, device, threads) persist to a plain
     JSON settings file in the platform config dir. Addresses are public
     data; no secret storage needed.

## Visual design (owner direction, 2026-08-13)

Minimalist terminal aesthetic. Black is the dominant color; the app should
read like a black terminal screen:

- Single dark theme only (no light mode). Background pure/near black
  (`#000`–`#0a0a0a`); text off-white/grey; monospaced type throughout.
- Accents stay monochrome (white/greys). At most one restrained highlight
  (e.g., Dinero orange) for the Start action and the connection dot; nothing
  else colored.
- **Background animation:** a subtle black-and-whitish "matrix / hash
  stream" — dim falling hex/hash characters behind the content, low
  contrast (greys on black), GPU-cheap (canvas, throttled frame rate,
  pauses when the window is hidden). It must never compete with the stats;
  when mining is stopped, it idles at even lower intensity.
- Hashrate is the hero element: large monospaced number, plain white.
- **Block-found detail (owner requirement):** when a block is found, the app
  shows the miner's full block output verbatim — hash, utreexo merkle root,
  nonce/extranonce, block height, and whatever else the binary prints —
  line-for-line in a "blocks" panel, in white on black, exactly like the CLI
  does today. The GUI adds no reformatting beyond monospace display.
- No cards, no shadows, no gradients — rules/dividers and spacing only.

## Data flow

Pool: UI → WorkSource resolves `dinero-stratum-worker` args → MinerProcess
spawns → worker connects to pool, authenticates with the payout address as
identity, submits shares → pool pays the address on its schedule → UI shows
shares/hashrate. The device holds no funds at any point.

Solo: UI → WorkSource resolves `dinero-miner`/`dinero-gpu-miner` args →
sidecar pulls block templates from the node RPC, builds a coinbase paying the
entered address, mines, submits found blocks → reward matures on-chain to the
address → UI shows hashrate/blocks found.

## Error handling

- Invalid address: inline, specific (bad checksum / wrong network / shielded).
- Pool unreachable / RPC unreachable: sidecar exits or logs errors →
  supervisor backoff + status banner; no data loss possible (nothing stored).
- GPU backend unavailable at Start (e.g., CUDA missing): sidecar error is
  parsed to a friendly message with the probed backends listed.
- Machine sleep/wake: treat resume like a reconnect (supervisor restarts the
  sidecar if it died).

## Packaging & distribution

- Tauri bundles: macOS `.dmg` (signed `Developer ID Application: DineroLabs
  LLC`, notarized + stapled via the same pipeline as DineroDPI), Windows
  installer, Linux AppImage/deb — published as GitHub releases with SHA-256
  sums, matching existing Dinero release conventions.
- Sidecar binaries are taken from the dinero-v8 release pipeline per
  platform (the same artifacts already shipped in dinero releases), pinned by
  version and checksum in the bundle manifest.

## Testing

- Unit: AddressValidator vectors (valid mainnet, bad checksum, tdin/rdin,
  shielded, garbage); per-binary log-line parsers over captured real output;
  WorkSource arg-resolution table tests (every matrix cell + greyed cell).
- Integration: on regtest (`-regtest`, RPC 20996), spawn the real sidecars
  against a local regtest node — solo CPU mines a block to a given address
  and the test asserts the coinbase pays it; pool path tested against a
  local SV2 pool instance from dinero-sv2.
- Manual matrix before each release: macOS/Metal, Windows/CUDA+OpenCL,
  Linux/OpenCL, CPU everywhere; per project rule, every regression fix ships
  with a failing-first test.

## Risks / open items

- Pool+GPU gap (upstream `dinero-gpu-miner` stratum support) — tracked as a
  dinero-v8 issue, not blocking v1.
- Remote-RPC credential support in the CLIs must be confirmed early in
  implementation (see WorkSource); patch upstream if absent.
- Windows CUDA build variance (memory: hidapi/SDK issues in past Windows
  lanes) — GPU on Windows validates during the manual matrix, OpenCL is the
  fallback backend.
