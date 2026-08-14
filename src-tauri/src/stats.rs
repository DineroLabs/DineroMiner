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
pub fn parse_line(line: &str, stats: &mut MinerStats) -> LineOutcome {
    let l = line.trim_end();
    let t = l.trim_start();

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
