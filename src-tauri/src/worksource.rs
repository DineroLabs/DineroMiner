use serde::{Deserialize, Serialize};

/// Default pool: the LIVE SV2 pool on the fleet (host = DineroDPI Contribute's
/// `defaultSV2PoolHost`, ContributeViewModel.swift:331; port 4444 = the
/// `dinero-sv2-pool` systemd deployment). The static Noise pubkey below is the
/// pool's pinned server key — without it the client accepts any server key on
/// first contact.
pub const DEFAULT_POOL_ENDPOINT: &str = "173.249.200.59:4444";
pub const DEFAULT_POOL_PUBKEY: &str =
    "3c879d90c9bb430493dfbf02cecbb93c3ae0d9d6c31d0757595e353fbe927417";

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum Mode {
    Pool,
    Solo,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum Device {
    Cpu,
    Gpu,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum GpuBackend {
    Auto,
    Metal,
    Cuda,
    Opencl,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MinerConfig {
    pub address: String,
    pub mode: Mode,
    pub device: Device,
    pub threads: Option<u32>, // None = binary auto-detect
    pub gpu_backend: GpuBackend,
    pub pool_endpoint: String, // "host:port"
    pub pool_pubkey: String,   // pool's static Noise pubkey (64-char hex)
    pub worker_name: String,
    pub solo_rpc_url: String, // e.g. "http://user:pass@host:20998"
}

#[derive(Debug, PartialEq)]
pub struct SidecarInvocation {
    pub program: &'static str,
    pub args: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub enum ResolveError {
    MissingSoloRpc,
    MissingPoolEndpoint,
    MissingPoolPubkey,
    NotTaproot,
}

pub fn resolve(cfg: &MinerConfig) -> Result<SidecarInvocation, ResolveError> {
    match (&cfg.mode, &cfg.device) {
        (Mode::Pool, dev) => {
            if cfg.pool_endpoint.trim().is_empty() {
                return Err(ResolveError::MissingPoolEndpoint);
            }
            if cfg.pool_pubkey.trim().is_empty() {
                return Err(ResolveError::MissingPoolPubkey);
            }
            let script = crate::address::payout_script_hex(&cfg.address)
                .map_err(|_| ResolveError::NotTaproot)?;
            let mut args = vec![
                "--pool".into(),
                cfg.pool_endpoint.clone(),
                "--server-pubkey".into(),
                cfg.pool_pubkey.trim().to_string(),
                "--payout-script-hex".into(),
                script,
                "--reward-mode".into(),
                "shared".into(),
                "--json".into(),
            ];
            if !cfg.worker_name.trim().is_empty() {
                args.extend(["--user-agent".into(), cfg.worker_name.trim().to_string()]);
            }
            match dev {
                Device::Cpu => {
                    if let Some(t) = cfg.threads {
                        args.extend(["--threads".into(), t.to_string()]);
                    }
                    Ok(SidecarInvocation { program: "dinero-sv2-miner", args })
                }
                Device::Gpu => {
                    let b = match cfg.gpu_backend {
                        GpuBackend::Auto => "auto",
                        GpuBackend::Metal => "metal",
                        GpuBackend::Cuda => "cuda",
                        GpuBackend::Opencl => "opencl",
                    };
                    args.extend(["--backend".into(), b.into()]);
                    Ok(SidecarInvocation { program: "dinero-sv2-gpu-miner", args })
                }
            }
        }
        (Mode::Solo, dev) => {
            if cfg.solo_rpc_url.trim().is_empty() {
                return Err(ResolveError::MissingSoloRpc);
            }
            let mut args = vec![
                "--rpc".into(),
                cfg.solo_rpc_url.clone(),
                "--address".into(),
                cfg.address.clone(),
            ];
            match dev {
                Device::Cpu => {
                    if let Some(t) = cfg.threads {
                        args.extend(["--threads".into(), t.to_string()]);
                    }
                    Ok(SidecarInvocation { program: "dinero-miner", args })
                }
                Device::Gpu => {
                    let b = match cfg.gpu_backend {
                        GpuBackend::Auto => "auto",
                        GpuBackend::Metal => "metal",
                        GpuBackend::Cuda => "cuda",
                        GpuBackend::Opencl => "opencl",
                    };
                    args.extend(["--backend".into(), b.into()]);
                    Ok(SidecarInvocation { program: "dinero-gpu-miner", args })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ADDR: &str = "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700";
    fn base() -> MinerConfig {
        MinerConfig {
            address: ADDR.into(),
            mode: Mode::Pool,
            device: Device::Cpu,
            threads: Some(4),
            gpu_backend: GpuBackend::Auto,
            pool_endpoint: "pool.example.org:4444".into(),
            pool_pubkey: "3c879d90c9bb430493dfbf02cecbb93c3ae0d9d6c31d0757595e353fbe927417".into(),
            worker_name: "rig1".into(),
            solo_rpc_url: "http://u:p@node.example.org:20998".into(),
        }
    }
    fn script() -> String {
        crate::address::payout_script_hex(ADDR).unwrap()
    }
    #[test]
    fn pool_cpu_maps_to_sv2_miner_shared() {
        let inv = resolve(&base()).unwrap();
        assert_eq!(inv.program, "dinero-sv2-miner");
        assert_eq!(
            inv.args,
            vec![
                "--pool".to_string(),
                "pool.example.org:4444".into(),
                "--server-pubkey".into(),
                "3c879d90c9bb430493dfbf02cecbb93c3ae0d9d6c31d0757595e353fbe927417".into(),
                "--payout-script-hex".into(),
                script(),
                "--reward-mode".into(),
                "shared".into(),
                "--json".into(),
                "--user-agent".into(),
                "rig1".into(),
                "--threads".into(),
                "4".into(),
            ]
        );
    }
    #[test]
    fn pool_cpu_auto_threads_omits_flag() {
        let mut c = base();
        c.threads = None;
        assert!(!resolve(&c).unwrap().args.contains(&"--threads".to_string()));
    }
    #[test]
    fn pool_gpu_maps_to_sv2_gpu_miner() {
        let mut c = base();
        c.device = Device::Gpu;
        c.gpu_backend = GpuBackend::Metal;
        let inv = resolve(&c).unwrap();
        assert_eq!(inv.program, "dinero-sv2-gpu-miner");
        assert!(inv.args.contains(&"--backend".to_string()));
        assert!(inv.args.contains(&"metal".to_string()));
        assert!(inv.args.contains(&"--json".to_string()));
        assert!(!inv.args.contains(&"--threads".to_string()));
    }
    #[test]
    fn pool_empty_worker_omits_user_agent() {
        let mut c = base();
        c.worker_name = "".into();
        let inv = resolve(&c).unwrap();
        assert!(!inv.args.contains(&"--user-agent".to_string()));
    }
    #[test]
    fn pool_missing_pubkey_is_error() {
        let mut c = base();
        c.pool_pubkey = "".into();
        assert_eq!(resolve(&c), Err(ResolveError::MissingPoolPubkey));
    }
    #[test]
    fn pool_non_taproot_address_is_error() {
        let hrp = bech32::Hrp::parse("din").unwrap();
        let mut c = base();
        c.address = bech32::segwit::encode(hrp, bech32::Fe32::Q, &[0u8; 20]).unwrap();
        assert_eq!(resolve(&c), Err(ResolveError::NotTaproot));
    }
    #[test]
    fn solo_cpu_maps_to_dinero_miner() {
        let mut c = base();
        c.mode = Mode::Solo;
        let inv = resolve(&c).unwrap();
        assert_eq!(inv.program, "dinero-miner");
        assert_eq!(
            inv.args,
            vec![
                "--rpc",
                "http://u:p@node.example.org:20998",
                "--address",
                "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700",
                "--threads",
                "4",
            ]
        );
    }
    #[test]
    fn solo_gpu_maps_to_gpu_miner_with_backend() {
        let mut c = base();
        c.mode = Mode::Solo;
        c.device = Device::Gpu;
        c.gpu_backend = GpuBackend::Metal;
        let inv = resolve(&c).unwrap();
        assert_eq!(inv.program, "dinero-gpu-miner");
        assert_eq!(
            inv.args,
            vec![
                "--rpc",
                "http://u:p@node.example.org:20998",
                "--address",
                "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700",
                "--backend",
                "metal",
            ]
        );
    }
    #[test]
    fn empty_solo_rpc_is_error() {
        let mut c = base();
        c.mode = Mode::Solo;
        c.solo_rpc_url = "".into();
        assert_eq!(resolve(&c), Err(ResolveError::MissingSoloRpc));
    }
    #[test]
    fn empty_pool_endpoint_is_error() {
        let mut c = base();
        c.pool_endpoint = "".into();
        assert_eq!(resolve(&c), Err(ResolveError::MissingPoolEndpoint));
    }
}
