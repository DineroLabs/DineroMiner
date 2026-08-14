use serde::Serialize;

#[derive(Clone, Default, Serialize, Debug)]
pub struct MinerStats {
    pub hashrate_hs: f64,
    pub shares_accepted: u64,
    pub shares_rejected: u64,
    pub blocks_found: u64,
    pub last_error: Option<String>,
}

#[derive(PartialEq, Debug)]
pub enum LineOutcome {
    Parsed,
    Unparsed,
    /// Block-found header + detail lines — forwarded verbatim to the UI's
    /// blocks panel (hash, merkle_root, utreexo_root, nonce, height, …).
    Block,
}

fn hashrate_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*([kKMG]?)H/s").unwrap())
}

fn counter(line: &str, key: &str) -> Option<u64> {
    let idx = line.find(key)?;
    line[idx + key.len()..]
        .trim_start_matches([':', ' '])
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|d| d.parse().ok())
}

/// Semantics: periodic stat lines carry authoritative totals (SET); discrete
/// event lines increment so the UI reacts immediately, and the next stat line
/// re-syncs. Box-drawing lines (╔ ║ ╚) are the miner's block-found display and
/// pass through verbatim as `Block`.
/// SV2 miners with `--json` emit one JSON object per line with an `"event"`
/// key (schema: dinero-sv2 `crates/dinero-sv2-miner/src/main.rs::emit`).
fn parse_json_event(t: &str, stats: &mut MinerStats) -> Option<LineOutcome> {
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let event = v.get("event")?.as_str()?;
    Some(match event {
        "hashrate" => {
            if let Some(mhs) = v.get("mhs").and_then(|m| m.as_f64()) {
                stats.hashrate_hs = mhs * 1e6;
            }
            LineOutcome::Parsed
        }
        "share_accepted" => {
            if let Some(n) = v.get("accepted_count").and_then(|n| n.as_u64()) {
                stats.shares_accepted = n;
            } else {
                stats.shares_accepted += 1;
            }
            LineOutcome::Parsed
        }
        "share_rejected" => {
            stats.shares_rejected += 1;
            LineOutcome::Parsed
        }
        "share_submitted" => {
            if v.get("meets_block_target").and_then(|b| b.as_bool()) == Some(true) {
                stats.blocks_found += 1;
                // The line carries the full solution (hash, nonce, tries) —
                // shown verbatim in the blocks panel.
                LineOutcome::Block
            } else {
                LineOutcome::Parsed
            }
        }
        "session_end" => {
            if v.get("reason").and_then(|r| r.as_str()) == Some("error") {
                stats.last_error = v
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|e| e.to_string())
                    .or_else(|| Some("session ended with error".into()));
            }
            LineOutcome::Parsed
        }
        // Connection/job lifecycle events carry no counters the UI shows.
        "startup" | "connected" | "channel_open" | "set_new_prev_hash" | "new_job"
        | "window_status" | "reconnect_wait" => LineOutcome::Parsed,
        _ => LineOutcome::Unparsed,
    })
}

pub fn parse_line(line: &str, stats: &mut MinerStats) -> LineOutcome {
    let l = line.trim_end();
    let t = l.trim_start();

    if t.starts_with('{') {
        if let Some(outcome) = parse_json_event(t, stats) {
            return outcome;
        }
    }

    // Block box: header increments, details display-only.
    if t.starts_with('╔') || t.starts_with('╚') || t.starts_with('║') {
        if t.contains("BLOCK FOUND") {
            stats.blocks_found += 1;
        }
        return LineOutcome::Block;
    }
    // Pool-side discrete block event.
    if t.contains("🧱 BLOCK FOUND") {
        stats.blocks_found += 1;
        return LineOutcome::Block;
    }
    // Acceptance confirmations display in the blocks panel but never re-count.
    let low = t.to_lowercase();
    if low.contains("block accepted") {
        return LineOutcome::Block;
    }

    // Periodic stat lines: hashrate + any counters present (authoritative).
    if let Some(c) = hashrate_regex().captures(t) {
        let v: f64 = c[1].parse().unwrap_or(0.0);
        stats.hashrate_hs = v
            * match &c[2] {
                "k" | "K" => 1e3,
                "M" => 1e6,
                "G" => 1e9,
                _ => 1.0,
            };
        if let Some(n) = counter(t, "Shares accepted") {
            stats.shares_accepted = n;
        }
        if let Some(n) = counter(t, "Shares rejected") {
            stats.shares_rejected = n;
        }
        if let Some(n) = counter(t, "Blocks") {
            stats.blocks_found = n.max(stats.blocks_found);
        }
        return LineOutcome::Parsed;
    }

    if low.contains("share") && low.contains("accept") {
        stats.shares_accepted += 1;
        return LineOutcome::Parsed;
    }
    if low.contains("share") && (low.contains("reject") || low.contains("stale")) {
        stats.shares_rejected += 1;
        return LineOutcome::Parsed;
    }
    if low.contains("error") || low.contains("failed") || low.contains("refused") || t.starts_with('❌') {
        stats.last_error = Some(t.to_string());
        return LineOutcome::Parsed;
    }
    LineOutcome::Unparsed
}

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
    fn solo_cpu_fixture_yields_blocks_and_parses_hashrate_lines() {
        let s = feed("tests/fixtures/dinero-miner.log");
        // Regtest legitimately reports 0.00 MH/s (blocks found instantly);
        // the GPU fixture asserts the >0 path. Here: blocks must be counted.
        assert!(s.blocks_found >= 1, "regtest run found no block in fixture");
        let mut t = MinerStats::default();
        assert_eq!(
            parse_line("⛏️  0.00 MH/s | Total: 0 MH | Blocks: 15", &mut t),
            LineOutcome::Parsed
        );
        assert_eq!(t.hashrate_hs, 0.0);
    }
    #[test]
    fn gpu_fixture_yields_positive_hashrate_and_blocks() {
        let s = feed("tests/fixtures/gpu-miner.log");
        assert!(s.hashrate_hs > 0.0, "no positive hashrate parsed from GPU fixture");
        assert!(s.blocks_found >= 1);
    }
    #[test]
    fn pool_fixture_yields_shares_and_error() {
        let s = feed("tests/fixtures/dinero-stratum-worker.log");
        assert!(s.shares_accepted >= 1, "no accepted shares parsed");
        assert!(s.shares_rejected >= 1, "no rejected shares parsed");
        assert!(s.last_error.is_some(), "connection-failure lines must surface an error");
    }
    #[test]
    fn hashrate_units_normalize_to_hs() {
        let mut s = MinerStats::default();
        parse_line("hashrate: 1.50 MH/s", &mut s);
        assert_eq!(s.hashrate_hs, 1_500_000.0);
        parse_line("⛏️  845.2 KH/s | Shares accepted: 5 | Shares rejected: 1 | Blocks: 1", &mut s);
        assert_eq!(s.hashrate_hs, 845_200.0);
        parse_line("GPU: 2 GH/s | Total: 1 MH | Blocks: 0", &mut s);
        assert_eq!(s.hashrate_hs, 2_000_000_000.0);
        parse_line("hashrate: 42 H/s", &mut s);
        assert_eq!(s.hashrate_hs, 42.0);
    }
    #[test]
    fn stat_line_counters_are_authoritative() {
        let mut s = MinerStats::default();
        parse_line("⛏️  1.23 MH/s | Shares accepted: 3 | Shares rejected: 1 | Blocks: 2", &mut s);
        assert_eq!(s.shares_accepted, 3);
        assert_eq!(s.shares_rejected, 1);
        assert_eq!(s.blocks_found, 2);
    }
    #[test]
    fn share_accepted_event_increments() {
        let mut s = MinerStats::default();
        parse_line("✅ Share accepted", &mut s);
        parse_line("✅ Share accepted", &mut s);
        assert_eq!(s.shares_accepted, 2);
    }
    #[test]
    fn unknown_lines_are_unparsed_not_errors() {
        let mut s = MinerStats::default();
        assert_eq!(parse_line("totally novel line", &mut s), LineOutcome::Unparsed);
        assert!(s.last_error.is_none());
    }
    #[test]
    fn block_found_sequence_yields_block_outcomes_with_details() {
        let text = std::fs::read_to_string("tests/fixtures/dinero-miner.log").unwrap();
        let mut s = MinerStats::default();
        let outcomes: Vec<(String, LineOutcome)> = text
            .lines()
            .map(|l| (l.to_string(), parse_line(l, &mut s)))
            .collect();
        let header = outcomes
            .iter()
            .position(|(l, o)| *o == LineOutcome::Block && l.contains("BLOCK FOUND"))
            .expect("no block header parsed");
        let detail_count = outcomes[header + 1..]
            .iter()
            .take_while(|(_, o)| *o == LineOutcome::Block)
            .count();
        assert!(detail_count >= 4, "block details (prev/merkle/utreexo/nBits) not captured");
        // Detail lines must include the utreexo root verbatim.
        let details: Vec<&str> = outcomes[header..header + detail_count + 1]
            .iter()
            .map(|(l, _)| l.as_str())
            .collect();
        assert!(details.iter().any(|l| l.contains("utreexo_root")));
    }
    #[test]
    fn sv2_json_fixture_yields_hashrate_and_the_mainnet_block() {
        // Captured live 2026-08-14 against the SJ pool — this session actually
        // found mainnet block 0000003a861a… (pool log: "SHARED block ACCEPTED").
        let s = feed("tests/fixtures/sv2-miner-json.log");
        assert!((s.hashrate_hs - 4_190_000.0).abs() < 50_000.0, "mhs 4.19 expected");
        assert_eq!(s.blocks_found, 1, "meets_block_target share = block found");
        assert!(s.last_error.is_none());
    }
    #[test]
    fn sv2_block_solution_line_is_block_outcome() {
        let mut s = MinerStats::default();
        let line = r#"{"event":"share_submitted","hash":"00000abc","meets_block_target":true,"nonce":"0x1","reward_mode":"shared","sequence_number":1,"tries":5}"#;
        assert_eq!(parse_line(line, &mut s), LineOutcome::Block);
        assert_eq!(s.blocks_found, 1);
        let plain = r#"{"event":"share_submitted","hash":"00000abc","meets_block_target":false,"nonce":"0x2","reward_mode":"shared","sequence_number":2,"tries":5}"#;
        assert_eq!(parse_line(plain, &mut s), LineOutcome::Parsed);
        assert_eq!(s.blocks_found, 1, "plain shares are not blocks");
    }
    #[test]
    fn sv2_share_accepted_count_is_authoritative() {
        let mut s = MinerStats::default();
        parse_line(r#"{"event":"share_accepted","accepted_count":7,"channel_id":2,"last_seq":9,"shares_sum":7}"#, &mut s);
        assert_eq!(s.shares_accepted, 7);
        parse_line(r#"{"event":"share_rejected","channel_id":2,"error":"stale-job","sequence_number":10}"#, &mut s);
        assert_eq!(s.shares_rejected, 1);
        assert!(s.last_error.is_none(), "a rejected share is not a session error");
    }
    #[test]
    fn sv2_session_end_error_sets_last_error() {
        let mut s = MinerStats::default();
        parse_line(r#"{"error":"noise handshake","event":"session_end","reason":"error"}"#, &mut s);
        assert_eq!(s.last_error.as_deref(), Some("noise handshake"));
        let mut t = MinerStats::default();
        parse_line(r#"{"event":"session_end","reason":"clean-close","blocks_found":0}"#, &mut t);
        assert!(t.last_error.is_none(), "clean close is not an error");
    }
    #[test]
    fn accepted_line_is_block_outcome_but_does_not_double_count() {
        let mut s = MinerStats::default();
        parse_line("║  BLOCK FOUND!  height=131  nonce=1", &mut s);
        assert_eq!(
            parse_line("Block ACCEPTED by daemon! (height 131)", &mut s),
            LineOutcome::Block
        );
        assert_eq!(s.blocks_found, 1, "ACCEPTED line must not increment again");
    }
}
