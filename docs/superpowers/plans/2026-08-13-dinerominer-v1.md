# DineroMiner v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the minimal cross-platform GUI miner: paste a Dinero address, pick CPU/GPU and Pool/Solo, press Start, watch hashrate — a Tauri shell supervising the existing dinero-v8 miner binaries.

**Architecture:** Rust core (address validation, sidecar arg resolution, process supervision, stat parsing, settings) exposed to a framework-free HTML/CSS/JS single screen via Tauri commands/events. Miner binaries run as supervised child processes; the app never implements hashing or protocols.

**Tech Stack:** Tauri 2.x, Rust (stable), `bech32` 0.11, `serde`/`serde_json`, tokio (Tauri's runtime), vanilla JS + `<canvas>` for the matrix background. No frontend framework.

## Global Constraints

- Local repo `~/src/DineroMiner`; GitHub `DineroLabs/DineroMiner` (public). Commit after every task.
- Spec: `docs/superpowers/specs/2026-08-13-miner-gui-design.md`. The support matrix is law: Pool→CPU only; Solo→CPU+GPU. Pool+GPU must resolve to a typed error, never a command.
- Mainnet addresses only: HRP `din`, bech32/bech32m, checksum-verified. `tdin`/`rdin`/`dins` rejected with specific errors.
- Single dark theme: background `#0a0a0a` or blacker, off-white monospace text, at most one orange accent (Start button, connection dot). No cards/shadows/gradients.
- Sidecars pinned by SHA-256 in `scripts/sidecars.lock`; app refuses to spawn a binary whose hash mismatches.
- No telemetry, no network calls except the miners' own.
- Every parser/validator behavior lands test-first (red before green — run the failing test before implementing).

---

### Task 1: Scaffold the Tauri project

**Files:**
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `ui/index.html`, `.gitignore`
- Test: `cargo test` runs (empty suite) and `cargo tauri dev` opens a black window.

**Interfaces:**
- Produces: crate `dinerominer` with `lib.rs` declaring `pub mod address; pub mod worksource; pub mod stats; pub mod supervisor; pub mod settings;` (modules added by later tasks; declare them as they land, not up front).

- [ ] **Step 1: Install tooling and scaffold**

```bash
cd ~/src/DineroMiner
cargo install tauri-cli --version '^2' --locked
mkdir -p src-tauri/src ui
```

- [ ] **Step 2: Write `src-tauri/Cargo.toml`**

```toml
[package]
name = "dinerominer"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bech32 = "0.11"
sha2 = "0.10"
hex = "0.4"

[lib]
name = "dinerominer"
crate-type = ["staticlib", "cdylib", "rlib"]
```

- [ ] **Step 3: Write `src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4: Write `src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "DineroMiner",
  "version": "0.1.0",
  "identifier": "org.dinerolabs.dinerominer",
  "build": { "frontendDist": "../ui" },
  "app": {
    "windows": [
      {
        "title": "DineroMiner",
        "width": 560,
        "height": 640,
        "minWidth": 480,
        "minHeight": 560,
        "backgroundColor": "#0a0a0a"
      }
    ],
    "security": { "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'" }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "resources": ["binaries/*"]
  }
}
```

- [ ] **Step 5: Write `src-tauri/src/main.rs` and `src-tauri/src/lib.rs`**

```rust
// main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    dinerominer::run()
}
```

```rust
// lib.rs
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running DineroMiner");
}
```

- [ ] **Step 6: Write placeholder `ui/index.html`**

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>DineroMiner</title>
    <style>body { background: #0a0a0a; }</style>
  </head>
  <body></body>
</html>
```

- [ ] **Step 7: Write `.gitignore`**

```
src-tauri/target/
src-tauri/binaries/
node_modules/
```

- [ ] **Step 8: Verify build**

Run: `cd src-tauri && cargo check && cargo test`
Expected: compiles, zero tests pass. Then `cargo tauri dev` — a black 560×640 window opens. Close it.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: scaffold Tauri app shell"
```

---

### Task 2: AddressValidator

**Files:**
- Create: `src-tauri/src/address.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod address;`)
- Test: inline `#[cfg(test)]` in `address.rs`

**Interfaces:**
- Produces: `pub fn validate_address(input: &str) -> Result<String, AddressError>` returning the trimmed, lowercased address; `#[derive(Debug, PartialEq, Serialize)] pub enum AddressError { Empty, Shielded, WrongNetwork, BadChecksum, Invalid }` with `impl AddressError { pub fn message(&self) -> &'static str }`.

- [ ] **Step 1: Write the failing tests**

```rust
// address.rs
#[cfg(test)]
mod tests {
    use super::*;
    // Real mainnet addresses (throwaway sim wallets from 2026-08-12 session).
    const VALID_1: &str = "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700";
    const VALID_2: &str = "din1p977z3vkm5a2skmvlfvng4lxd9mnv95z43a38pastawrnc89gu7xsfcyczw";

    #[test]
    fn accepts_real_mainnet_addresses() {
        assert_eq!(validate_address(VALID_1).unwrap(), VALID_1);
        assert_eq!(validate_address(VALID_2).unwrap(), VALID_2);
    }
    #[test]
    fn trims_and_lowercases() {
        let shouty = format!("  {}  ", VALID_1.to_uppercase());
        assert_eq!(validate_address(&shouty).unwrap(), VALID_1);
    }
    #[test]
    fn rejects_empty() {
        assert_eq!(validate_address("   "), Err(AddressError::Empty));
    }
    #[test]
    fn rejects_bad_checksum() {
        let mut s = VALID_1.to_string();
        s.pop();
        s.push('q'); // corrupt final checksum char
        assert_eq!(validate_address(&s), Err(AddressError::BadChecksum));
    }
    #[test]
    fn rejects_shielded_prefix() {
        assert_eq!(validate_address("dins1qqqqqq"), Err(AddressError::Shielded));
    }
    #[test]
    fn rejects_testnet_and_regtest() {
        assert_eq!(validate_address("tdin1qqqqqq"), Err(AddressError::WrongNetwork));
        assert_eq!(validate_address("rdin1qqqqqq"), Err(AddressError::WrongNetwork));
    }
    #[test]
    fn rejects_garbage() {
        assert_eq!(validate_address("hello world"), Err(AddressError::Invalid));
        assert_eq!(validate_address("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"), Err(AddressError::Invalid));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test address -- --nocapture` (from `src-tauri/`)
Expected: FAIL — `validate_address` not found.

- [ ] **Step 3: Implement**

```rust
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
pub enum AddressError { Empty, Shielded, WrongNetwork, BadChecksum, Invalid }

impl AddressError {
    pub fn message(&self) -> &'static str {
        match self {
            AddressError::Empty => "Enter a Dinero address.",
            AddressError::Shielded => "Shielded (dins1…) addresses can't receive mining payouts. Use a transparent din1… address.",
            AddressError::WrongNetwork => "That's a testnet/regtest address. Enter a mainnet din1… address.",
            AddressError::BadChecksum => "Checksum doesn't match — the address has a typo.",
            AddressError::Invalid => "Not a valid Dinero address.",
        }
    }
}

pub fn validate_address(input: &str) -> Result<String, AddressError> {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        return Err(AddressError::Empty);
    }
    // HRP-specific rejections read the prefix before the LAST '1' separator.
    if let Some(idx) = s.rfind('1') {
        match &s[..idx] {
            "dins" => return Err(AddressError::Shielded),
            "tdin" | "rdin" => return Err(AddressError::WrongNetwork),
            _ => {}
        }
    }
    match bech32::decode(&s) {
        Ok((hrp, _data)) if hrp.as_str() == "din" => Ok(s),
        Ok(_) => Err(AddressError::Invalid),
        Err(e) => {
            // The bech32 crate reports checksum failures distinctly; everything
            // else (bad chars, mixed case, length) is Invalid.
            let msg = e.to_string().to_lowercase();
            if msg.contains("checksum") { Err(AddressError::BadChecksum) } else { Err(AddressError::Invalid) }
        }
    }
}
```

Add `pub mod address;` to `lib.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test address`
Expected: all 7 PASS. If `rejects_bad_checksum` maps to `Invalid` instead of `BadChecksum`, inspect the crate's error `Debug` output in the test and match on the concrete `DecodeError` variant instead of the string — the test defines the required behavior, adjust the implementation, not the test.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: mainnet address validator with typed errors"
```

---

### Task 3: WorkSource — config → sidecar invocation

**Files:**
- Create: `src-tauri/src/worksource.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod worksource;`)
- Test: inline `#[cfg(test)]`

**Interfaces:**
- Consumes: nothing (pure).
- Produces:

```rust
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum Mode { Pool, Solo }
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum Device { Cpu, Gpu }
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum GpuBackend { Auto, Metal, Cuda, Opencl }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MinerConfig {
    pub address: String,
    pub mode: Mode,
    pub device: Device,
    pub threads: Option<u32>,       // None = binary auto-detect
    pub gpu_backend: GpuBackend,
    pub pool_endpoint: String,      // "host:port"
    pub worker_name: String,
    pub solo_rpc_url: String,       // e.g. "http://user:pass@host:20998"
}

#[derive(Debug, PartialEq)]
pub struct SidecarInvocation { pub program: &'static str, pub args: Vec<String> }

#[derive(Debug, PartialEq, Serialize)]
pub enum ResolveError { PoolGpuUnsupported, MissingSoloRpc, MissingPoolEndpoint }

pub fn resolve(cfg: &MinerConfig) -> Result<SidecarInvocation, ResolveError>;
pub const DEFAULT_POOL_ENDPOINT: &str = /* Step 1 */;
```

- [ ] **Step 1: Copy the real pool endpoint from DineroDPI's Contribute config**

Run: `grep -rn -i -E "stratum|pool" --include="*.swift" ~/src/apps/DineroDPI/DineroDPI/DineroDPI/Core/ | grep -i -E "endpoint|host|url|default" | head -20`
Take the production stratum host:port the Contribute feature uses and set it as `DEFAULT_POOL_ENDPOINT`. Record the source file/line in a code comment.

- [ ] **Step 2: Write the failing table tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> MinerConfig {
        MinerConfig {
            address: "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700".into(),
            mode: Mode::Pool, device: Device::Cpu, threads: Some(4),
            gpu_backend: GpuBackend::Auto,
            pool_endpoint: "pool.example.org:3333".into(),
            worker_name: "rig1".into(),
            solo_rpc_url: "http://u:p@node.example.org:20998".into(),
        }
    }
    #[test]
    fn pool_cpu_maps_to_stratum_worker() {
        let inv = resolve(&base()).unwrap();
        assert_eq!(inv.program, "dinero-stratum-worker");
        assert_eq!(inv.args, vec![
            "--stratum", "pool.example.org:3333",
            "--user", "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700.rig1",
            "--threads", "4",
        ]);
    }
    #[test]
    fn pool_cpu_auto_threads_omits_flag() {
        let mut c = base(); c.threads = None;
        assert!(!resolve(&c).unwrap().args.contains(&"--threads".to_string()));
    }
    #[test]
    fn pool_gpu_is_a_typed_error() {
        let mut c = base(); c.device = Device::Gpu;
        assert_eq!(resolve(&c), Err(ResolveError::PoolGpuUnsupported));
    }
    #[test]
    fn solo_cpu_maps_to_dinero_miner() {
        let mut c = base(); c.mode = Mode::Solo;
        let inv = resolve(&c).unwrap();
        assert_eq!(inv.program, "dinero-miner");
        assert_eq!(inv.args, vec![
            "--rpc", "http://u:p@node.example.org:20998",
            "--address", "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700",
            "--threads", "4",
        ]);
    }
    #[test]
    fn solo_gpu_maps_to_gpu_miner_with_backend() {
        let mut c = base(); c.mode = Mode::Solo; c.device = Device::Gpu; c.gpu_backend = GpuBackend::Metal;
        let inv = resolve(&c).unwrap();
        assert_eq!(inv.program, "dinero-gpu-miner");
        assert_eq!(inv.args, vec![
            "--rpc", "http://u:p@node.example.org:20998",
            "--address", "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700",
            "--backend", "metal",
        ]);
    }
    #[test]
    fn empty_solo_rpc_is_error() {
        let mut c = base(); c.mode = Mode::Solo; c.solo_rpc_url = "".into();
        assert_eq!(resolve(&c), Err(ResolveError::MissingSoloRpc));
    }
    #[test]
    fn empty_pool_endpoint_is_error() {
        let mut c = base(); c.pool_endpoint = "".into();
        assert_eq!(resolve(&c), Err(ResolveError::MissingPoolEndpoint));
    }
    #[test]
    fn empty_worker_name_uses_bare_address_as_user() {
        let mut c = base(); c.worker_name = "".into();
        let inv = resolve(&c).unwrap();
        assert!(inv.args.contains(&c.address));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test worksource`
Expected: FAIL — module missing.

- [ ] **Step 4: Implement `resolve`**

```rust
pub fn resolve(cfg: &MinerConfig) -> Result<SidecarInvocation, ResolveError> {
    match (&cfg.mode, &cfg.device) {
        (Mode::Pool, Device::Gpu) => Err(ResolveError::PoolGpuUnsupported),
        (Mode::Pool, Device::Cpu) => {
            if cfg.pool_endpoint.trim().is_empty() { return Err(ResolveError::MissingPoolEndpoint); }
            let user = if cfg.worker_name.trim().is_empty() {
                cfg.address.clone()
            } else {
                format!("{}.{}", cfg.address, cfg.worker_name.trim())
            };
            let mut args = vec!["--stratum".into(), cfg.pool_endpoint.clone(), "--user".into(), user];
            if let Some(t) = cfg.threads { args.extend(["--threads".into(), t.to_string()]); }
            Ok(SidecarInvocation { program: "dinero-stratum-worker", args })
        }
        (Mode::Solo, dev) => {
            if cfg.solo_rpc_url.trim().is_empty() { return Err(ResolveError::MissingSoloRpc); }
            let mut args = vec!["--rpc".into(), cfg.solo_rpc_url.clone(), "--address".into(), cfg.address.clone()];
            match dev {
                Device::Cpu => {
                    if let Some(t) = cfg.threads { args.extend(["--threads".into(), t.to_string()]); }
                    Ok(SidecarInvocation { program: "dinero-miner", args })
                }
                Device::Gpu => {
                    let b = match cfg.gpu_backend {
                        GpuBackend::Auto => "auto", GpuBackend::Metal => "metal",
                        GpuBackend::Cuda => "cuda", GpuBackend::Opencl => "opencl",
                    };
                    args.extend(["--backend".into(), b.into()]);
                    Ok(SidecarInvocation { program: "dinero-gpu-miner", args })
                }
            }
        }
    }
}
```

- [ ] **Step 5: Run tests to verify all pass, commit**

Run: `cargo test worksource` → all PASS.

```bash
git add -A && git commit -m "feat: worksource resolver with support-matrix enforcement"
```

---

### Task 4: Capture real miner output fixtures

**Files:**
- Create: `src-tauri/tests/fixtures/dinero-miner.log`, `src-tauri/tests/fixtures/dinero-stratum-worker.log`, `src-tauri/tests/fixtures/gpu-miner.log`, `src-tauri/tests/fixtures/README.md`

**Interfaces:**
- Produces: raw captured stdout/stderr samples that Task 5's parser tests consume. Fixture README records exactly how each was captured.

- [ ] **Step 1: Capture solo CPU output on regtest**

```bash
B=~/src/dinero-v8/build
D=/tmp/dinerominer-regtest && rm -rf $D && mkdir -p $D
$B/dinerod -regtest -datadir=$D &            # RPC defaults to 20996 on regtest
sleep 5
ADDR=$($B/dinero-cli -regtest -datadir=$D getnewaddress)
timeout 90 $B/dinero-miner --rpcport 20996 --datadir $D --address $ADDR --threads 2 --force \
  2>&1 | tee src-tauri/tests/fixtures/dinero-miner.log
```

Expected: the log contains startup lines, periodic hashrate lines, and (regtest difficulty is minimal) at least one found/submitted-block line within the 90 s window.

- [ ] **Step 2: Capture GPU miner output**

Same node, run: `timeout 60 $B/dinero-gpu-miner --rpcport 20996 --datadir $D --address $ADDR --backend metal --force 2>&1 | tee src-tauri/tests/fixtures/gpu-miner.log`
If Metal init fails on this machine, keep the failure output as the fixture — the parser must handle backend-failure lines too, and note it in the fixtures README.

- [ ] **Step 3: Capture pool worker output against the production pool**

Run (2 minutes is enough for connect + a few shares; use a real address you control):
`timeout 120 $B/dinero-stratum-worker --stratum <DEFAULT_POOL_ENDPOINT from Task 3> --user din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700.fixture --threads 2 2>&1 | tee src-tauri/tests/fixtures/dinero-stratum-worker.log`
Also capture a connection-failure sample: run once with `--stratum 127.0.0.1:1` for ~10 s and append its output to the same file under a `--- connection failure ---` marker line.

- [ ] **Step 4: Stop the regtest node, write fixtures README, commit**

```bash
$B/dinero-cli -regtest -datadir=$D stop || pkill -f "dinerod -regtest"
```

`fixtures/README.md`: one paragraph per fixture — binary version (`--version` output), exact command, date. Commit:

```bash
git add -A && git commit -m "test: captured real miner output fixtures"
```

---

### Task 5: Stats — line parsers over the fixtures

**Files:**
- Create: `src-tauri/src/stats.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod stats;`)
- Test: inline `#[cfg(test)]` reading `tests/fixtures/*`

**Interfaces:**
- Consumes: Task 4 fixtures.
- Produces:

```rust
#[derive(Clone, Default, Serialize, Debug)]
pub struct MinerStats {
    pub hashrate_hs: f64,
    pub shares_accepted: u64,
    pub shares_rejected: u64,
    pub blocks_found: u64,
    pub last_error: Option<String>,
}
#[derive(PartialEq, Debug)]
pub enum LineOutcome { Parsed, Unparsed, Block }
/// `Block` marks the block-found header line AND its detail lines (hash,
/// utreexo merkle root, nonce/extranonce, height, …) so the supervisor can
/// forward them verbatim to the UI's blocks panel. blocks_found increments
/// only on the header line. parse_line tracks block context internally via
/// `stats.in_block_context: bool` (public field on MinerStats, default
/// false): set on the header line, cleared on the first line that matches
/// no block-detail pattern.
pub fn parse_line(line: &str, stats: &mut MinerStats) -> LineOutcome;
```

`MinerStats` additionally carries `pub in_block_context: bool` (serde-skipped;
parser bookkeeping only). Block-detail patterns are keyed off the fixture: a
line inside block context returns `Block` if it contains any of
`hash`, `merkle`, `utreexo`, `nonce`, `height`, `target`, `difficulty`
(case-insensitive) — adjust the keyword list to the exact labels in
`tests/fixtures/dinero-miner.log`, which regtest guarantees contains at least
one real block-found sequence.

- [ ] **Step 1: Write the failing fixture-driven tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    fn feed(path: &str) -> MinerStats {
        let mut s = MinerStats::default();
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let _ = parse_line(line, &mut s);
        }
        s
    }
    #[test]
    fn solo_cpu_fixture_yields_hashrate_and_a_block() {
        let s = feed("tests/fixtures/dinero-miner.log");
        assert!(s.hashrate_hs > 0.0, "no hashrate parsed");
        assert!(s.blocks_found >= 1, "regtest run found no block in fixture");
    }
    #[test]
    fn pool_fixture_yields_hashrate_and_shares() {
        let s = feed("tests/fixtures/dinero-stratum-worker.log");
        assert!(s.hashrate_hs > 0.0);
        assert!(s.shares_accepted >= 1);
    }
    #[test]
    fn pool_fixture_connection_failure_sets_error() {
        let s = feed("tests/fixtures/dinero-stratum-worker.log");
        assert!(s.last_error.is_some(), "connection-failure section must surface an error");
    }
    #[test]
    fn hashrate_units_normalize_to_hs() {
        let mut s = MinerStats::default();
        parse_line("hashrate: 1.50 MH/s", &mut s);
        assert_eq!(s.hashrate_hs, 1_500_000.0);
        parse_line("hashrate: 800 kH/s", &mut s);
        assert_eq!(s.hashrate_hs, 800_000.0);
        parse_line("hashrate: 42 H/s", &mut s);
        assert_eq!(s.hashrate_hs, 42.0);
    }
    #[test]
    fn unknown_lines_are_unparsed_not_errors() {
        let mut s = MinerStats::default();
        assert_eq!(parse_line("totally novel line", &mut s), LineOutcome::Unparsed);
        assert!(s.last_error.is_none());
    }
    #[test]
    fn block_found_sequence_yields_block_outcomes_with_details() {
        // Every line of the real block-found sequence in the solo fixture must
        // come back as Block so the UI can show it verbatim. Locate the header
        // line and assert it plus at least two following detail lines.
        let text = std::fs::read_to_string("tests/fixtures/dinero-miner.log").unwrap();
        let mut s = MinerStats::default();
        let outcomes: Vec<(String, LineOutcome)> = text.lines()
            .map(|l| (l.to_string(), parse_line(l, &mut s))).collect();
        let header = outcomes.iter().position(|(l, o)| *o == LineOutcome::Block
            && l.to_lowercase().contains("block")).expect("no block header parsed");
        let detail_count = outcomes[header + 1..].iter()
            .take_while(|(_, o)| *o == LineOutcome::Block).count();
        assert!(detail_count >= 2, "block details (hash/merkle/nonce/height) not captured");
        assert_eq!(s.blocks_found, 1, "details must not double-count blocks");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test stats`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement `parse_line` against the captured formats**

Read the three fixture files first, then implement. Starting patterns (adjust ONLY the regex literals to match the fixtures — the test assertions define the contract):

```rust
pub fn parse_line(line: &str, stats: &mut MinerStats) -> LineOutcome {
    let l = line.trim();
    // Hashrate: any "<num> <unit>H/s" occurrence; normalize unit prefix.
    if let Some(c) = regex_captures(l, r"([0-9]+(?:\.[0-9]+)?)\s*([kMG]?)H/s") {
        let v: f64 = c.0.parse().unwrap_or(0.0);
        stats.hashrate_hs = v * match c.1 { "k" => 1e3, "M" => 1e6, "G" => 1e9, _ => 1.0 };
        return LineOutcome::Parsed;
    }
    let low = l.to_lowercase();
    if low.contains("share") && low.contains("accept") { stats.shares_accepted += 1; return LineOutcome::Parsed; }
    if low.contains("share") && (low.contains("reject") || low.contains("stale")) { stats.shares_rejected += 1; return LineOutcome::Parsed; }
    if (low.contains("block") && (low.contains("found") || low.contains("submitted") || low.contains("accepted")))
        && !low.contains("share") { stats.blocks_found += 1; return LineOutcome::Parsed; }
    if low.contains("error") || low.contains("failed") || low.contains("refused") {
        stats.last_error = Some(l.to_string());
        return LineOutcome::Parsed;
    }
    LineOutcome::Unparsed
}
```

Implement `regex_captures` with the `regex` crate (add `regex = "1"` to Cargo.toml) as a small helper returning the two capture groups. Where a fixture format contradicts a starting pattern (e.g., block lines that would double-count, or share lines that carry totals rather than events), fix the implementation so the fixture tests pass truthfully — never weaken a fixture assertion to make a wrong parser pass.

- [ ] **Step 4: Run tests to verify all pass, commit**

Run: `cargo test stats` → all PASS.

```bash
git add -A && git commit -m "feat: miner output parsers grounded in captured fixtures"
```

---

### Task 6: Supervisor — spawn, stream, restart, crash-loop stop

**Files:**
- Create: `src-tauri/src/supervisor.rs`, `src-tauri/tests/fake_miner.sh`
- Modify: `src-tauri/src/lib.rs` (add `pub mod supervisor;`)
- Test: inline `#[cfg(test)]` using the fake sidecar script

**Interfaces:**
- Consumes: `stats::{MinerStats, parse_line, LineOutcome}`.
- Produces:

```rust
#[derive(Clone, Serialize, Debug)]
pub enum MinerEvent {
    Stats(MinerStats),
    RawLine(String),          // unparsed lines, verbatim, for the raw-log view
    BlockLine(String),        // block-found header + detail lines, verbatim, for the blocks panel
    Status(String),           // "starting" | "running" | "reconnecting" | "stopped" | "crash-loop"
}
pub struct Supervisor { /* handle + shutdown flag */ }
impl Supervisor {
    /// program is an absolute path (caller resolves sidecar dir + hash check).
    pub fn start(program: std::path::PathBuf, args: Vec<String>,
                 tx: std::sync::mpsc::Sender<MinerEvent>) -> Supervisor;
    pub fn stop(&self);       // kills the child, joins the thread
}
pub const BACKOFF_START_SECS: u64 = 2;
pub const BACKOFF_CAP_SECS: u64 = 60;
pub const CRASH_LOOP_EXITS: u32 = 5;
pub const CRASH_LOOP_WINDOW_SECS: u64 = 120;
```

Implementation notes: plain `std::thread` + `std::process::Command` with piped stdout/stderr (merged), `BufReader::lines()`; a `AtomicBool` shutdown flag checked in the restart loop; emit `Stats` after every `Parsed` line, `RawLine` for `Unparsed`, and for `Block` outcomes emit `BlockLine(line)` followed by `Stats` (the header line increments `blocks_found`, so stats stay current in the same breath).

- [ ] **Step 1: Write `tests/fake_miner.sh` (the test sidecar)**

```bash
#!/bin/sh
# Modes via $1: "steady" prints stats forever; "crash" exits immediately;
# "burst" prints 3 lines then exits 0.
case "$1" in
  steady) while true; do echo "hashrate: 123 kH/s"; echo "share accepted"; sleep 0.1; done ;;
  burst)  echo "hashrate: 5 H/s"; echo "unknown gibberish"; echo "share accepted"; exit 0 ;;
  crash)  echo "error: boom" >&2; exit 1 ;;
esac
```

`chmod +x src-tauri/tests/fake_miner.sh`

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    fn fake() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake_miner.sh")
    }
    fn drain(rx: &std::sync::mpsc::Receiver<MinerEvent>, ms: u64) -> Vec<MinerEvent> {
        let deadline = std::time::Instant::now() + Duration::from_millis(ms);
        let mut out = vec![];
        while std::time::Instant::now() < deadline {
            if let Ok(e) = rx.recv_timeout(Duration::from_millis(50)) { out.push(e); }
        }
        out
    }
    #[test]
    fn streams_stats_and_raw_lines() {
        let (tx, rx) = channel();
        let s = Supervisor::start(fake(), vec!["burst".into()], tx);
        let events = drain(&rx, 1500);
        s.stop();
        assert!(events.iter().any(|e| matches!(e, MinerEvent::Stats(st) if st.hashrate_hs == 5.0)));
        assert!(events.iter().any(|e| matches!(e, MinerEvent::RawLine(l) if l.contains("gibberish"))));
    }
    #[test]
    fn stop_terminates_steady_child() {
        let (tx, rx) = channel();
        let s = Supervisor::start(fake(), vec!["steady".into()], tx);
        let _ = drain(&rx, 500);
        s.stop();
        let after = drain(&rx, 700);
        assert!(after.iter().all(|e| !matches!(e, MinerEvent::Stats(_))), "stats kept flowing after stop");
    }
    #[test]
    fn crash_loop_emits_crash_loop_status() {
        // Compile-time-overridable knobs keep this test fast: with window/backoff
        // shrunk via cfg(test) consts, 5 instant exits must land within the window.
        let (tx, rx) = channel();
        let s = Supervisor::start(fake(), vec!["crash".into()], tx);
        let events = drain(&rx, 4000);
        s.stop();
        assert!(events.iter().any(|e| matches!(e, MinerEvent::Status(st) if st == "crash-loop")));
        assert!(events.iter().any(|e| matches!(e, MinerEvent::Stats(st) if st.last_error.as_deref() == Some("error: boom"))));
    }
}
```

Under `#[cfg(test)]`, define `BACKOFF_START_SECS=0`, `CRASH_LOOP_WINDOW_SECS=10` (use `cfg!(test)` ternaries at the use sites) so the crash-loop test completes in seconds.

- [ ] **Step 3: Run tests to verify they fail** — `cargo test supervisor` → FAIL (module missing).

- [ ] **Step 4: Implement the supervisor loop**

Single spawned thread: loop { emit Status("starting"); spawn child with merged stdout/stderr; emit Status("running"); read lines → `parse_line` → emit `Stats(clone)` or `RawLine`; on child exit: record exit time in a `VecDeque<Instant>`, trim to window, if len ≥ CRASH_LOOP_EXITS emit Status("crash-loop") and break; if shutdown flag set break; else emit Status("reconnecting"), sleep backoff (doubling, capped), continue }. `stop()`: set flag, kill child (store child id behind a Mutex), join.

- [ ] **Step 5: Run tests to verify all pass, commit**

Run: `cargo test supervisor` → PASS (3/3).

```bash
git add -A && git commit -m "feat: sidecar supervisor with backoff and crash-loop stop"
```

---

### Task 7: Settings persistence

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod settings;`)
- Test: inline `#[cfg(test)]` with a temp dir

**Interfaces:**
- Consumes: `worksource::{Mode, Device, GpuBackend}`.
- Produces:

```rust
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Settings {
    pub address: String,
    pub mode: Mode,
    pub device: Device,
    pub threads: Option<u32>,
    pub gpu_backend: GpuBackend,
    pub pool_endpoint: String,
    pub worker_name: String,
    pub solo_rpc_url: String,
}
impl Default for Settings { /* Pool, Cpu, threads None, Auto, DEFAULT_POOL_ENDPOINT, hostname, "" */ }
pub fn load(dir: &std::path::Path) -> Settings;              // missing/corrupt file → Default
pub fn save(dir: &std::path::Path, s: &Settings) -> std::io::Result<()>;  // writes settings.json
```

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("dm-set-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Settings::default();
        s.address = "din1test".into();
        s.threads = Some(6);
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir), s);
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn missing_file_yields_defaults() {
        let dir = std::env::temp_dir().join(format!("dm-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load(&dir), Settings::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn corrupt_file_yields_defaults() {
        let dir = std::env::temp_dir().join(format!("dm-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), b"{not json").unwrap();
        assert_eq!(load(&dir), Settings::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn default_worker_name_is_hostname_or_fallback() {
        let d = Settings::default();
        assert!(!d.worker_name.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify FAIL, implement, run to verify PASS**

Implementation: `serde_json::to_string_pretty` / `from_str` with `.unwrap_or_default()`; hostname via reading `hostname::get()` (add crate `hostname = "0.4"`) with fallback `"worker"`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: JSON settings persistence with safe defaults"
```

---

### Task 8: Sidecar integrity + Tauri command wiring

**Files:**
- Create: `src-tauri/src/commands.rs`, `scripts/fetch-sidecars.sh`, `scripts/sidecars.lock`
- Modify: `src-tauri/src/lib.rs` (add `pub mod commands;`, register commands + managed state)
- Test: inline `#[cfg(test)]` for the hash gate; command handlers stay thin wrappers

**Interfaces:**
- Consumes: everything from Tasks 2–7.
- Produces (Tauri commands the UI calls, and events it listens to):
  - `validate_address_cmd(input: String) -> Result<String, AddressError>`
  - `get_settings() -> Settings` / `save_settings(s: Settings) -> Result<(), String>`
  - `start_mining(s: Settings) -> Result<(), String>` — validates address, resolves invocation, hash-checks the sidecar, starts the supervisor, forwards `MinerEvent`s as Tauri events `"miner-event"`; also persists settings.
  - `stop_mining() -> ()`
  - `pub fn verify_sidecar(path: &Path, lock: &str) -> Result<(), String>` — SHA-256 of the binary must appear in `sidecars.lock` (format: `<sha256>  <filename>` lines).

- [ ] **Step 1: Write `scripts/fetch-sidecars.sh` and the lock file**

```bash
#!/usr/bin/env bash
# Populates src-tauri/binaries/ for the current platform and (re)writes the
# entries in scripts/sidecars.lock. v1 source: the local dinero-v8 build for
# macOS; Linux/Windows artifacts come from the dinero release tarballs and are
# added to this script when those bundles are wired (documented gap until then).
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p src-tauri/binaries
SRC="${DINERO_BUILD_DIR:-$HOME/src/dinero-v8/build}"
for b in dinero-miner dinero-gpu-miner dinero-stratum-worker; do
  cp "$SRC/$b" src-tauri/binaries/
  shasum -a 256 "src-tauri/binaries/$b" | sed 's|src-tauri/binaries/||' >> scripts/sidecars.lock.new
done
sort -u scripts/sidecars.lock.new > scripts/sidecars.lock && rm scripts/sidecars.lock.new
echo "sidecars staged; lock updated"
```

Run it once; commit the lock file (binaries stay gitignored).

- [ ] **Step 2: Write the failing hash-gate test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verify_sidecar_accepts_matching_hash_and_rejects_tampering() {
        let dir = std::env::temp_dir().join(format!("dm-sc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("dinero-miner");
        std::fs::write(&bin, b"binary-bytes").unwrap();
        let hash = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(b"binary-bytes"))
        };
        let lock = format!("{}  dinero-miner\n", hash);
        assert!(verify_sidecar(&bin, &lock).is_ok());
        std::fs::write(&bin, b"tampered").unwrap();
        assert!(verify_sidecar(&bin, &lock).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
```

- [ ] **Step 3: Run to verify FAIL, implement `verify_sidecar` + command handlers, run to verify PASS**

Command wiring in `lib.rs`: managed state `Mutex<Option<Supervisor>>`; `start_mining` = `validate_address` → `worksource::resolve` → locate binary in the resource dir (dev override env `DINEROMINER_SIDECAR_DIR`) → `verify_sidecar` with the lock file bundled as a resource → `Supervisor::start` with an mpsc whose receiver thread forwards each `MinerEvent` via `app.emit("miner-event", &e)`. `stop_mining` takes the supervisor out of the state and calls `.stop()`. Register all five commands in `tauri::generate_handler!`.

- [ ] **Step 4: Run the full suite and a manual smoke**

Run: `cargo test` → all tasks' tests PASS.
Run: `DINEROMINER_SIDECAR_DIR=$PWD/src-tauri/binaries cargo tauri dev`, open the devtools console and call `window.__TAURI__.core.invoke('validate_address_cmd', { input: 'din1qq' })` → rejection object arrives.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: sidecar hash gate and Tauri command surface"
```

---

### Task 9: UI — terminal-black single screen

**Files:**
- Create: `ui/style.css`, `ui/app.js`; rewrite `ui/index.html`
- Test: manual visual + behavior checklist (below); pure display helpers unit-tested in Task 10's harness are not needed — keep `app.js` free of logic beyond formatting

**Interfaces:**
- Consumes: the five commands + `"miner-event"` event from Task 8. `formatHashrate(hs)` produced here is reused by nothing else — keep it local.

- [ ] **Step 1: Write `ui/index.html`**

```html
<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <title>DineroMiner</title>
  <link rel="stylesheet" href="style.css" />
</head>
<body>
  <canvas id="matrix"></canvas>
  <main>
    <header>DINEROMINER</header>

    <label for="address">payout address</label>
    <div class="row">
      <input id="address" spellcheck="false" placeholder="din1…" />
      <button id="paste" title="Paste">⎘</button>
      <span id="addr-status"></span>
    </div>
    <div id="addr-error" class="error"></div>

    <div class="row seg" id="mode">
      <button data-v="pool" class="on">POOL</button><button data-v="solo">SOLO</button>
    </div>
    <div id="solo-row" class="hidden">
      <label for="rpc">node rpc url</label>
      <input id="rpc" spellcheck="false" placeholder="http://user:pass@host:20998" />
      <div class="note">a remote server can waste your hashes but cannot steal a found reward — the coinbase pays the address above</div>
    </div>

    <div class="row seg" id="device">
      <button data-v="cpu" class="on">CPU</button><button data-v="gpu">GPU</button>
    </div>
    <div id="cpu-row" class="row">
      <label for="threads">threads</label>
      <input id="threads" type="range" min="1" max="16" value="4" /><span id="threads-n">4</span>
    </div>
    <div id="gpu-row" class="row hidden">
      <label for="backend">backend</label>
      <select id="backend"></select>
    </div>
    <div id="matrix-gap" class="error hidden">pool + gpu isn’t supported yet — the gpu miner speaks node rpc only</div>

    <button id="start">START</button>

    <section id="stats" class="hidden">
      <div id="hashrate">0 H/s</div>
      <div class="row small">
        <span id="conn" class="dot"></span><span id="status">stopped</span>
        <span id="shares"></span><span id="uptime"></span>
      </div>
    </section>

    <section id="blocks" class="hidden">
      <label>blocks found</label>
      <pre id="blocklog"></pre>
    </section>

    <details><summary>advanced</summary>
      <label for="pool-endpoint">pool endpoint</label>
      <input id="pool-endpoint" spellcheck="false" />
      <label for="worker">worker name</label>
      <input id="worker" spellcheck="false" />
      <pre id="rawlog"></pre>
    </details>
  </main>
  <script src="matrix.js"></script>
  <script src="app.js"></script>
</body>
</html>
```

- [ ] **Step 2: Write `ui/style.css` (the whole theme)**

```css
* { margin: 0; padding: 0; box-sizing: border-box; }
:root {
  --bg: #0a0a0a; --fg: #d8d8d0; --dim: #6a6a64; --line: #222;
  --accent: #f7931a; /* the single allowed accent */
}
html, body { height: 100%; }
body {
  background: var(--bg); color: var(--fg);
  font: 13px/1.6 "SF Mono", "Cascadia Mono", "JetBrains Mono", monospace;
  overflow: hidden;
}
#matrix { position: fixed; inset: 0; z-index: 0; }
main { position: relative; z-index: 1; max-width: 460px; margin: 0 auto; padding: 28px 20px; }
header { letter-spacing: 6px; color: var(--dim); border-bottom: 1px solid var(--line);
         padding-bottom: 12px; margin-bottom: 20px; }
label { display: block; color: var(--dim); text-transform: lowercase; margin: 14px 0 4px; }
input, select, button, pre {
  font: inherit; color: var(--fg); background: transparent;
  border: 1px solid var(--line); padding: 8px 10px;
}
input:focus, select:focus { outline: none; border-color: var(--dim); }
.row { display: flex; gap: 8px; align-items: center; }
.row input { flex: 1; }
.seg { margin-top: 14px; }
.seg button { flex: 1; color: var(--dim); cursor: pointer; }
.seg button.on { color: var(--fg); border-color: var(--dim); }
.seg button:disabled { opacity: .35; cursor: not-allowed; }
.error { color: #c96a6a; min-height: 1.4em; margin-top: 4px; }
.note { color: var(--dim); margin-top: 4px; }
.hidden { display: none !important; }
#start { width: 100%; margin-top: 20px; padding: 12px;
         color: var(--accent); border-color: var(--accent);
         letter-spacing: 4px; cursor: pointer; }
#start:disabled { color: var(--dim); border-color: var(--line); cursor: not-allowed; }
#start.running { color: var(--bg); background: var(--accent); }
#stats { margin-top: 24px; border-top: 1px solid var(--line); padding-top: 16px; }
#hashrate { font-size: 40px; color: #fff; }
.small { color: var(--dim); margin-top: 6px; gap: 14px; }
.dot { width: 8px; height: 8px; border-radius: 50%; background: var(--dim); display: inline-block; }
.dot.on { background: var(--accent); }
details { margin-top: 24px; color: var(--dim); }
details input { width: 100%; }
#rawlog { margin-top: 10px; max-height: 140px; overflow-y: auto; color: var(--dim);
          border-color: var(--line); white-space: pre-wrap; }
#blocks { margin-top: 20px; }
#blocklog { max-height: 180px; overflow-y: auto; color: #e6e6e0;
            border-color: var(--line); white-space: pre-wrap; }
```

- [ ] **Step 3: Write `ui/app.js`**

```javascript
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

const state = { settings: null, running: false };

function formatHashrate(hs) {
  if (hs >= 1e9) return (hs / 1e9).toFixed(2) + " GH/s";
  if (hs >= 1e6) return (hs / 1e6).toFixed(2) + " MH/s";
  if (hs >= 1e3) return (hs / 1e3).toFixed(2) + " kH/s";
  return Math.round(hs) + " H/s";
}

function collectSettings() {
  return {
    address: $("address").value.trim(),
    mode: $("#mode .on") ? document.querySelector("#mode .on").dataset.v === "solo" ? "Solo" : "Pool" : "Pool",
    device: document.querySelector("#device .on").dataset.v === "gpu" ? "Gpu" : "Cpu",
    threads: parseInt($("threads").value, 10),
    gpu_backend: $("backend").value || "Auto",
    pool_endpoint: $("pool-endpoint").value.trim(),
    worker_name: $("worker").value.trim(),
    solo_rpc_url: $("rpc").value.trim(),
  };
}

function applyMatrixGap() {
  const mode = document.querySelector("#mode .on").dataset.v;
  const gpuBtn = document.querySelector('#device button[data-v="gpu"]');
  const gap = mode === "pool";
  gpuBtn.disabled = gap;
  if (gap && gpuBtn.classList.contains("on")) selectSeg("device", "cpu");
  $("matrix-gap").classList.toggle("hidden", !(gap && gpuBtn.matches(":hover")));
}

function selectSeg(groupId, v) {
  document.querySelectorAll(`#${groupId} button`).forEach(b =>
    b.classList.toggle("on", b.dataset.v === v));
  $("solo-row").classList.toggle("hidden",
    document.querySelector("#mode .on").dataset.v !== "solo");
  const gpu = document.querySelector("#device .on").dataset.v === "gpu";
  $("cpu-row").classList.toggle("hidden", gpu);
  $("gpu-row").classList.toggle("hidden", !gpu);
  applyMatrixGap();
}

async function validate() {
  const input = $("address").value;
  try {
    await invoke("validate_address_cmd", { input });
    $("addr-status").textContent = "✓";
    $("addr-error").textContent = "";
    $("start").disabled = false;
  } catch (e) {
    $("addr-status").textContent = input.trim() ? "✗" : "";
    $("addr-error").textContent = input.trim() ? (e.message || messageFor(e)) : "";
    $("start").disabled = true;
  }
}
function messageFor(err) {
  const m = { Empty: "", Shielded: "shielded (dins1…) addresses can't receive payouts",
    WrongNetwork: "testnet/regtest address — mainnet din1… required",
    BadChecksum: "checksum mismatch — typo in the address",
    Invalid: "not a valid dinero address" };
  return m[err] ?? String(err);
}

async function start() {
  const s = collectSettings();
  try {
    await invoke("start_mining", { s });
    state.running = true;
    $("start").textContent = "STOP";
    $("start").classList.add("running");
    $("stats").classList.remove("hidden");
  } catch (e) { $("addr-error").textContent = String(e); }
}
async function stop() {
  await invoke("stop_mining");
  state.running = false;
  $("start").textContent = "START";
  $("start").classList.remove("running");
  $("status").textContent = "stopped";
  $("conn").classList.remove("on");
  window.matrixIntensity(0.3);
}

window.addEventListener("DOMContentLoaded", async () => {
  state.settings = await invoke("get_settings");
  const s = state.settings;
  $("address").value = s.address; $("rpc").value = s.solo_rpc_url;
  $("threads").value = s.threads ?? 4; $("threads-n").textContent = $("threads").value;
  $("pool-endpoint").value = s.pool_endpoint; $("worker").value = s.worker_name;
  const backends = navigator.platform.startsWith("Mac") ? ["Auto", "Metal"] : ["Auto", "Cuda", "Opencl"];
  $("backend").innerHTML = backends.map(b => `<option>${b}</option>`).join("");
  selectSeg("mode", s.mode === "Solo" ? "solo" : "pool");
  selectSeg("device", s.device === "Gpu" ? "gpu" : "cpu");
  validate();

  $("address").addEventListener("input", validate);
  $("paste").addEventListener("click", async () => {
    $("address").value = await navigator.clipboard.readText();
    validate();
  });
  $("threads").addEventListener("input", () => $("threads-n").textContent = $("threads").value);
  document.querySelectorAll("#mode button").forEach(b =>
    b.addEventListener("click", () => selectSeg("mode", b.dataset.v)));
  document.querySelectorAll("#device button").forEach(b =>
    b.addEventListener("click", () => { if (!b.disabled) selectSeg("device", b.dataset.v); }));
  $("start").addEventListener("click", () => state.running ? stop() : start());

  let startedAt = null;
  await listen("miner-event", (ev) => {
    const e = ev.payload;
    if (e.Stats) {
      $("hashrate").textContent = formatHashrate(e.Stats.hashrate_hs);
      $("shares").textContent =
        `ok ${e.Stats.shares_accepted} / rej ${e.Stats.shares_rejected}` +
        (e.Stats.blocks_found ? ` / blocks ${e.Stats.blocks_found}` : "");
      if (e.Stats.last_error) $("addr-error").textContent = e.Stats.last_error;
    } else if (e.BlockLine) {
      // Verbatim block output: hash, utreexo merkle root, nonce, height, …
      // exactly as the miner prints it.
      $("blocks").classList.remove("hidden");
      const bl = $("blocklog");
      bl.textContent = (bl.textContent + "\n" + e.BlockLine).split("\n").slice(-400).join("\n");
      bl.scrollTop = bl.scrollHeight;
    } else if (e.RawLine) {
      const log = $("rawlog");
      log.textContent = (log.textContent + "\n" + e.RawLine).split("\n").slice(-200).join("\n");
    } else if (e.Status) {
      $("status").textContent = e.Status;
      $("conn").classList.toggle("on", e.Status === "running");
      if (e.Status === "running") { startedAt = Date.now(); window.matrixIntensity(1.0); }
      if (e.Status === "crash-loop") stop();
    }
  });
  setInterval(() => {
    if (state.running && startedAt)
      $("uptime").textContent = Math.floor((Date.now() - startedAt) / 1000) + "s";
  }, 1000);
});
```

- [ ] **Step 4: Manual behavior checklist (run `cargo tauri dev` with `DINEROMINER_SIDECAR_DIR` set)**

- [ ] Empty address → Start disabled, no error shown
- [ ] Paste a valid `din1…` → ✓, Start enabled; corrupt last char → ✗ "checksum mismatch"
- [ ] `dins1x`/`tdin1x` → specific messages
- [ ] Mode POOL → GPU button disabled; SOLO → GPU selectable, RPC field + trust note visible
- [ ] Start (pool/CPU, real pool endpoint) → status dot orange, hashrate ticks, shares count
- [ ] Stop → child gone (`pgrep dinero-stratum-worker` empty), status "stopped"
- [ ] Advanced → raw log fills with unparsed lines
- [ ] Solo on regtest (point RPC at a local regtest node, `--force`-free path once peers exist, or reuse Task 11's node) → on block found, the blocks panel appears and shows the miner's full verbatim block output (hash, utreexo merkle root, nonce, height, …) in near-white

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: terminal-black single-screen UI"
```

---

### Task 10: Matrix background animation

**Files:**
- Create: `ui/matrix.js`
- Test: manual (frame-rate + pause behavior), part of Task 9's checklist rerun

**Interfaces:**
- Produces: `window.matrixIntensity(v)` — 0.3 idle (default), 1.0 while mining; consumed by `app.js` (already wired in Task 9).

- [ ] **Step 1: Write `ui/matrix.js`**

```javascript
(() => {
  const canvas = document.getElementById("matrix");
  const ctx = canvas.getContext("2d");
  const CHARS = "0123456789abcdef";
  const FONT = 12, FPS = 14;
  let cols = [], intensity = 0.3, timer = null;

  function resize() {
    canvas.width = innerWidth; canvas.height = innerHeight;
    const n = Math.floor(innerWidth / FONT);
    cols = Array.from({ length: n }, () => Math.floor(Math.random() * innerHeight / FONT));
  }
  function frame() {
    ctx.fillStyle = "rgba(10,10,10,0.12)";           // trails fade into bg
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.font = FONT + "px monospace";
    for (let i = 0; i < cols.length; i++) {
      if (Math.random() > intensity * 0.75) continue; // intensity thins the rain
      const ch = CHARS[Math.floor(Math.random() * CHARS.length)];
      const head = Math.random() < 0.04;
      ctx.fillStyle = head ? "rgba(230,230,225,0.8)" : "rgba(120,120,115,0.28)";
      ctx.fillText(ch, i * FONT, cols[i] * FONT);
      cols[i] = cols[i] * FONT > canvas.height && Math.random() > 0.975 ? 0 : cols[i] + 1;
    }
  }
  function run() { clearInterval(timer); timer = setInterval(frame, 1000 / FPS); }
  document.addEventListener("visibilitychange", () =>
    document.hidden ? clearInterval(timer) : run());
  addEventListener("resize", resize);
  window.matrixIntensity = (v) => { intensity = v; };
  resize(); run();
})();
```

- [ ] **Step 2: Manual verification**

- [ ] Rain is dim grey with occasional white heads, clearly behind the content, unreadable-noise level — the hashrate number dominates
- [ ] `window.matrixIntensity(1.0)` in devtools visibly densifies it; `0.3` calms it
- [ ] Hide the window/tab → CPU usage of the app drops (Activity Monitor); reshow → resumes
- [ ] Resize keeps full-bleed coverage

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: black-and-white matrix rain background"
```

---

### Task 11: Regtest end-to-end — solo mining pays the entered address

**Files:**
- Create: `scripts/e2e-regtest.sh`
- Test: the script IS the test; exits non-zero on failure

**Interfaces:**
- Consumes: dinero-v8 build dir (`DINERO_BUILD_DIR`, default `~/src/dinero-v8/build`).

- [ ] **Step 1: Write `scripts/e2e-regtest.sh`**

```bash
#!/usr/bin/env bash
# E2E: solo-mine one regtest block via the same sidecar+args the app resolves,
# then assert the coinbase pays the target address.
set -euo pipefail
B="${DINERO_BUILD_DIR:-$HOME/src/dinero-v8/build}"
D=$(mktemp -d /tmp/dm-e2e.XXXXXX)
cleanup() { "$B/dinero-cli" -regtest -datadir="$D" stop >/dev/null 2>&1 || true; sleep 1; rm -rf "$D"; }
trap cleanup EXIT

"$B/dinerod" -regtest -datadir="$D" & sleep 5
ADDR=$("$B/dinero-cli" -regtest -datadir="$D" getnewaddress)
H0=$("$B/dinero-cli" -regtest -datadir="$D" getblockcount)

timeout 120 "$B/dinero-miner" --rpcport 20996 --datadir "$D" --address "$ADDR" --threads 2 --force &
MINER=$!
for i in $(seq 1 60); do
  H=$("$B/dinero-cli" -regtest -datadir="$D" getblockcount)
  [ "$H" -gt "$H0" ] && break
  sleep 2
done
kill $MINER 2>/dev/null || true
[ "$H" -gt "$H0" ] || { echo "FAIL: no block mined"; exit 1; }

HASH=$("$B/dinero-cli" -regtest -datadir="$D" getbestblockhash)
BLOCK=$("$B/dinero-cli" -regtest -datadir="$D" getblock "$HASH" 2)
echo "$BLOCK" | grep -q "$ADDR" || { echo "FAIL: coinbase does not pay $ADDR"; echo "$BLOCK" | head -40; exit 1; }
echo "PASS: block $H coinbase pays $ADDR"
```

`chmod +x scripts/e2e-regtest.sh`. If `getblock <hash> 2`'s verbosity level or output shape differs on this chain, adapt the assertion to whatever RPC exposes the coinbase outputs (`dinero-cli -regtest help getblock` is authoritative) — the assertion "coinbase pays $ADDR" itself is non-negotiable.

- [ ] **Step 2: Run it**

Run: `./scripts/e2e-regtest.sh`
Expected: `PASS: block 1 coinbase pays rdin1…` (regtest difficulty makes this fast).

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "test: regtest e2e — solo coinbase pays the entered address"
```

---

### Task 12: Package for macOS + release checklist

**Files:**
- Create: `docs/RELEASING.md`
- Modify: `src-tauri/tauri.conf.json` (bundle identity)

**Interfaces:**
- Consumes: everything; produces the shippable artifact.

- [ ] **Step 1: Add macOS signing to `tauri.conf.json`**

Add inside `"bundle"`:

```json
"macOS": {
  "signingIdentity": "Developer ID Application: DineroLabs LLC (JXJS6ZA5FJ)",
  "minimumSystemVersion": "13.0"
}
```

- [ ] **Step 2: Build and verify the bundle**

Run:
```bash
./scripts/fetch-sidecars.sh
cargo tauri build
codesign -dvvv src-tauri/target/release/bundle/macos/DineroMiner.app 2>&1 | grep Authority | head -1
spctl -a -vvv src-tauri/target/release/bundle/macos/DineroMiner.app
```
Expected: `Authority=Developer ID Application: DineroLabs LLC (JXJS6ZA5FJ)`. (`spctl` will reject until notarized — that's expected pre-notarization.) Launch the built app and rerun Task 9's manual checklist once against it.

- [ ] **Step 3: Write `docs/RELEASING.md`**

Content: the exact notarize/staple commands mirroring DineroDPI's flow (`xcrun notarytool submit <dmg> --keychain-profile dinero-notarytool --wait`, `xcrun stapler staple`), the GitHub-release convention (tag `vX.Y.Z`, DMG + SHA-256 in notes, mark Latest), and the documented gaps: Windows/Linux sidecar sourcing in `fetch-sidecars.sh`, and the upstream pool+GPU issue link (file it in dinero-v8 and paste the URL here).

- [ ] **Step 4: Commit and push**

```bash
git add -A && git commit -m "build: macOS signing config and release runbook"
git push
```

---

## Self-Review (performed at write time)

- **Spec coverage:** validator (T2), matrix enforcement (T3), parsers grounded in real output (T4+T5), supervisor/backoff/crash-loop/raw-log (T6), settings persistence (T7), sidecar pinning + command surface (T8), single-screen terminal-black UI incl. trust note, greyed pool+GPU, paste, advanced/raw log (T9), matrix rain with visibility-pause and intensity (T10), regtest coinbase-pays-address integration (T11), packaging/signing + release runbook (T12). Pool endpoint default sourced from Contribute (T3 Step 1). Remote-RPC credential verification happens naturally at T9's manual pool/solo smoke — if `--rpc http://user:pass@…` fails, the upstream patch flagged in the spec becomes a blocking follow-up task; noted in RELEASING.md gaps.
- **Placeholder scan:** the two "adapt to observed reality" points (T5 regexes, T11 getblock verbosity) each pin the non-negotiable contract in tests and name the authoritative source — not placeholders.
- **Type consistency:** `MinerStats`/`MinerEvent` names match across T5/T6/T8/T9 (`Stats`, `RawLine`, `BlockLine`, `Status` payload keys used by `app.js` match the serde enum serialization of `MinerEvent`; `LineOutcome::Block` in T5 drives `BlockLine` emission in T6 and the blocks panel in T9); `Settings` fields match `MinerConfig` field-for-field except `SidecarInvocation` internals, which only T8 touches; commands invoked in `app.js` (`validate_address_cmd`, `get_settings`, `start_mining`, `stop_mining`) match T8's list (`save_settings` is called inside `start_mining` — the UI never calls it directly, acceptable and consistent).
