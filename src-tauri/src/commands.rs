use crate::address::validate_address;
use crate::settings::{self, Settings};
use crate::supervisor::{MinerEvent, Supervisor};
use crate::worksource::{self, MinerConfig};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct MinerState(pub Mutex<Option<Supervisor>>);

/// SHA-256 of the binary must appear in sidecars.lock ("<sha256>  <filename>"
/// lines). The app refuses to spawn anything it can't verify.
pub fn verify_sidecar(path: &Path, lock: &str) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "sidecar path has no file name".to_string())?;
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read sidecar {name}: {e}"))?;
    let hash = hex::encode(Sha256::digest(&bytes));
    let expected = lock
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?, it.next()?))
        })
        .find(|(_, n)| *n == name)
        .map(|(h, _)| h.to_string())
        .ok_or_else(|| format!("sidecar {name} not present in sidecars.lock"))?;
    if hash != expected {
        return Err(format!(
            "sidecar {name} hash mismatch: refusing to run a modified binary"
        ));
    }
    Ok(())
}

fn sidecar_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(dev) = std::env::var("DINEROMINER_SIDECAR_DIR") {
        return Ok(PathBuf::from(dev));
    }
    app.path()
        .resolve("binaries", tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("cannot resolve resource dir: {e}"))
}

fn lock_contents(app: &AppHandle) -> Result<String, String> {
    if let Ok(dev) = std::env::var("DINEROMINER_LOCK_FILE") {
        return std::fs::read_to_string(dev).map_err(|e| e.to_string());
    }
    let bundled = app
        .path()
        .resolve("binaries/sidecars.lock", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    std::fs::read_to_string(bundled).map_err(|e| format!("sidecars.lock unreadable: {e}"))
}

fn config_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("dinerominer"))
}

#[tauri::command]
pub fn validate_address_cmd(input: String) -> Result<String, String> {
    validate_address(&input).map_err(|e| e.message().to_string())
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    settings::load(&config_dir(&app))
}

#[tauri::command]
pub fn save_settings(app: AppHandle, s: Settings) -> Result<(), String> {
    settings::save(&config_dir(&app), &s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_mining(app: AppHandle, state: State<MinerState>, s: Settings) -> Result<(), String> {
    let address = validate_address(&s.address).map_err(|e| e.message().to_string())?;
    let cfg = MinerConfig {
        address,
        mode: s.mode.clone(),
        device: s.device.clone(),
        threads: s.threads,
        gpu_backend: s.gpu_backend.clone(),
        pool_endpoint: s.pool_endpoint.clone(),
        worker_name: s.worker_name.clone(),
        solo_rpc_url: s.solo_rpc_url.clone(),
    };
    let inv = worksource::resolve(&cfg).map_err(|e| match e {
        worksource::ResolveError::PoolGpuUnsupported => {
            "Pool + GPU isn't supported yet — the GPU miner speaks node RPC only.".to_string()
        }
        worksource::ResolveError::MissingSoloRpc => "Enter a node RPC URL for solo mining.".to_string(),
        worksource::ResolveError::MissingPoolEndpoint => "Enter a pool endpoint.".to_string(),
    })?;
    let dir = sidecar_dir(&app)?;
    let program = dir.join(inv.program);
    verify_sidecar(&program, &lock_contents(&app)?)?;

    let mut slot = state.0.lock().unwrap();
    if let Some(old) = slot.take() {
        old.stop();
    }
    let _ = settings::save(&config_dir(&app), &s);

    let (tx, rx) = std::sync::mpsc::channel::<MinerEvent>();
    let emitter = app.clone();
    std::thread::spawn(move || {
        for ev in rx {
            let _ = emitter.emit("miner-event", &ev);
        }
    });
    *slot = Some(Supervisor::start(program, inv.args, tx));
    Ok(())
}

#[tauri::command]
pub fn stop_mining(state: State<MinerState>) {
    if let Some(sup) = state.0.lock().unwrap().take() {
        sup.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verify_sidecar_accepts_matching_hash_and_rejects_tampering() {
        let dir = std::env::temp_dir().join(format!("dm-sc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("dinero-miner");
        std::fs::write(&bin, b"binary-bytes").unwrap();
        let hash = hex::encode(Sha256::digest(b"binary-bytes"));
        let lock = format!("{}  dinero-miner\n", hash);
        assert!(verify_sidecar(&bin, &lock).is_ok());
        std::fs::write(&bin, b"tampered").unwrap();
        assert!(verify_sidecar(&bin, &lock).is_err());
        let missing = format!("{}  some-other-binary\n", hash);
        assert!(verify_sidecar(&bin, &missing).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
