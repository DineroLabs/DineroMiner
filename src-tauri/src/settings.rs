use crate::worksource::{Device, GpuBackend, Mode, DEFAULT_POOL_ENDPOINT, DEFAULT_POOL_PUBKEY};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Settings {
    pub address: String,
    pub mode: Mode,
    pub device: Device,
    pub threads: Option<u32>,
    pub gpu_backend: GpuBackend,
    pub pool_endpoint: String,
    #[serde(default = "default_pool_pubkey")]
    pub pool_pubkey: String,
    pub worker_name: String,
    pub solo_rpc_url: String,
}

fn default_pool_pubkey() -> String {
    DEFAULT_POOL_PUBKEY.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            address: String::new(),
            mode: Mode::Pool,
            device: Device::Cpu,
            threads: None,
            gpu_backend: GpuBackend::Auto,
            pool_endpoint: DEFAULT_POOL_ENDPOINT.to_string(),
            pool_pubkey: DEFAULT_POOL_PUBKEY.to_string(),
            worker_name: hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .filter(|h| !h.is_empty())
                .unwrap_or_else(|| "worker".to_string()),
            solo_rpc_url: String::new(),
        }
    }
}

/// Missing or corrupt file → defaults; settings are recoverable public data.
pub fn load(dir: &Path) -> Settings {
    std::fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, s: &Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_string_pretty(s).expect("settings serialize"),
    )
}

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
    #[test]
    fn default_pool_endpoint_is_the_contribute_default() {
        assert_eq!(Settings::default().pool_endpoint, crate::worksource::DEFAULT_POOL_ENDPOINT);
    }
}
