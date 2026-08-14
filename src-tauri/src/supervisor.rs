use crate::stats::{parse_line, LineOutcome, MinerStats};
use serde::Serialize;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Serialize, Debug)]
pub enum MinerEvent {
    Stats(MinerStats),
    /// Unparsed lines, verbatim, for the raw-log view.
    RawLine(String),
    /// Block-found header + detail lines, verbatim, for the blocks panel.
    BlockLine(String),
    /// "starting" | "running" | "reconnecting" | "stopped" | "crash-loop"
    Status(String),
}

pub const BACKOFF_START_SECS: u64 = if cfg!(test) { 0 } else { 2 };
pub const BACKOFF_CAP_SECS: u64 = 60;
pub const CRASH_LOOP_EXITS: u32 = 5;
pub const CRASH_LOOP_WINDOW_SECS: u64 = if cfg!(test) { 10 } else { 120 };

pub struct Supervisor {
    shutdown: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Supervisor {
    /// `program` is an absolute path — the caller resolves the sidecar dir and
    /// verifies its hash before handing it over.
    pub fn start(program: PathBuf, args: Vec<String>, tx: Sender<MinerEvent>) -> Supervisor {
        let shutdown = Arc::new(AtomicBool::new(false));
        let child_slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let (sd, cs) = (shutdown.clone(), child_slot.clone());

        let handle = std::thread::spawn(move || {
            let mut stats = MinerStats::default();
            let mut exits: VecDeque<Instant> = VecDeque::new();
            let mut backoff = BACKOFF_START_SECS;
            loop {
                if sd.load(Ordering::SeqCst) {
                    break;
                }
                let _ = tx.send(MinerEvent::Status("starting".into()));
                let spawned = Command::new(&program)
                    .args(&args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn();
                let mut child = match spawned {
                    Ok(c) => c,
                    Err(e) => {
                        stats.last_error = Some(format!("failed to launch miner: {e}"));
                        let _ = tx.send(MinerEvent::Stats(stats.clone()));
                        let _ = tx.send(MinerEvent::Status("crash-loop".into()));
                        break;
                    }
                };
                let _ = tx.send(MinerEvent::Status("running".into()));

                // Merge stderr into the same line stream via a helper thread.
                let stderr = child.stderr.take();
                let stdout = child.stdout.take();
                *cs.lock().unwrap() = Some(child);
                let (line_tx, line_rx) = std::sync::mpsc::channel::<String>();
                let mut readers = vec![];
                if let Some(out) = stdout {
                    let ltx = line_tx.clone();
                    readers.push(std::thread::spawn(move || {
                        for line in BufReader::new(out).lines().map_while(Result::ok) {
                            if ltx.send(line).is_err() {
                                break;
                            }
                        }
                    }));
                }
                if let Some(err) = stderr {
                    let ltx = line_tx.clone();
                    readers.push(std::thread::spawn(move || {
                        for line in BufReader::new(err).lines().map_while(Result::ok) {
                            if ltx.send(line).is_err() {
                                break;
                            }
                        }
                    }));
                }
                drop(line_tx);
                for line in line_rx {
                    if sd.load(Ordering::SeqCst) {
                        break;
                    }
                    match parse_line(&line, &mut stats) {
                        LineOutcome::Parsed => {
                            let _ = tx.send(MinerEvent::Stats(stats.clone()));
                        }
                        LineOutcome::Block => {
                            let _ = tx.send(MinerEvent::BlockLine(line));
                            let _ = tx.send(MinerEvent::Stats(stats.clone()));
                        }
                        LineOutcome::Unparsed => {
                            let _ = tx.send(MinerEvent::RawLine(line));
                        }
                    }
                }
                for r in readers {
                    let _ = r.join();
                }
                if let Some(mut c) = cs.lock().unwrap().take() {
                    let _ = c.wait();
                }
                if sd.load(Ordering::SeqCst) {
                    break;
                }
                // Unexpected exit: crash-loop detection, then backoff restart.
                let now = Instant::now();
                exits.push_back(now);
                while exits
                    .front()
                    .is_some_and(|t| now.duration_since(*t).as_secs() > CRASH_LOOP_WINDOW_SECS)
                {
                    exits.pop_front();
                }
                if exits.len() as u32 >= CRASH_LOOP_EXITS {
                    let _ = tx.send(MinerEvent::Status("crash-loop".into()));
                    break;
                }
                let _ = tx.send(MinerEvent::Status("reconnecting".into()));
                std::thread::sleep(Duration::from_secs(backoff));
                backoff = if cfg!(test) {
                    0
                } else {
                    (backoff.max(1) * 2).min(BACKOFF_CAP_SECS)
                };
            }
            let _ = tx.send(MinerEvent::Status("stopped".into()));
        });

        Supervisor {
            shutdown,
            child: child_slot,
            handle: Mutex::new(Some(handle)),
        }
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.lock().unwrap().as_mut() {
            let _ = child.kill();
        }
        if let Some(h) = self.handle.lock().unwrap().take() {
            let _ = h.join();
        }
    }
}

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
            if let Ok(e) = rx.recv_timeout(Duration::from_millis(50)) {
                out.push(e);
            }
        }
        out
    }
    #[test]
    fn streams_stats_and_raw_lines() {
        let (tx, rx) = channel();
        let s = Supervisor::start(fake(), vec!["burst".into()], tx);
        let events = drain(&rx, 1500);
        s.stop();
        assert!(events
            .iter()
            .any(|e| matches!(e, MinerEvent::Stats(st) if st.hashrate_hs == 5.0)));
        assert!(events
            .iter()
            .any(|e| matches!(e, MinerEvent::RawLine(l) if l.contains("gibberish"))));
    }
    #[test]
    fn stop_terminates_steady_child() {
        let (tx, rx) = channel();
        let s = Supervisor::start(fake(), vec!["steady".into()], tx);
        let _ = drain(&rx, 500);
        s.stop();
        // Allow in-flight events to flush, then require silence.
        let _ = drain(&rx, 300);
        let after = drain(&rx, 700);
        assert!(
            after.iter().all(|e| !matches!(e, MinerEvent::Stats(_))),
            "stats kept flowing after stop"
        );
    }
    #[test]
    fn crash_loop_emits_crash_loop_status() {
        let (tx, rx) = channel();
        let s = Supervisor::start(fake(), vec!["crash".into()], tx);
        let events = drain(&rx, 4000);
        s.stop();
        assert!(events
            .iter()
            .any(|e| matches!(e, MinerEvent::Status(st) if st == "crash-loop")));
        assert!(events
            .iter()
            .any(|e| matches!(e, MinerEvent::Stats(st) if st.last_error.as_deref() == Some("error: boom"))));
    }
}

#[cfg(test)]
mod live_probe {
    use super::*;
    /// Diagnostic: drive the REAL sv2 miner through the real Supervisor for
    /// 15 s and print every event. Run explicitly:
    ///   cargo test live_probe -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_miner_events_flow() {
        let (tx, rx) = std::sync::mpsc::channel();
        let s = Supervisor::start(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries/dinero-sv2-miner"),
            vec![
                "--pool".into(), "173.249.200.59:4444".into(),
                "--server-pubkey".into(),
                "3c879d90c9bb430493dfbf02cecbb93c3ae0d9d6c31d0757595e353fbe927417".into(),
                "--payout-script-hex".into(),
                "5120ea448139c9c82c9bfb95b583938085f232488b0c26cd4cf36611ab4dbce0d2d0".into(),
                "--reward-mode".into(), "shared".into(),
                "--user-agent".into(), "supervisor-probe".into(),
                "--threads".into(), "1".into(),
                "--json".into(),
            ],
            tx,
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut n = 0u32;
        while std::time::Instant::now() < deadline {
            if let Ok(e) = rx.recv_timeout(std::time::Duration::from_millis(200)) {
                n += 1;
                println!("EVT {n}: {e:?}");
            }
        }
        s.stop();
        assert!(n > 3, "no events flowed from the real miner");
    }
}
