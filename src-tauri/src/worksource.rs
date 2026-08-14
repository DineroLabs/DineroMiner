use serde::{Deserialize, Serialize};

/// Default pool endpoint, copied from the shipped DineroDPI Contribute
/// feature: host from ContributeViewModel.swift:331 (`defaultSV2PoolHost`),
/// port from its legacy stratum default at ContributeViewModel.swift:519.
pub const DEFAULT_POOL_ENDPOINT: &str = "173.249.200.59:3333";

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
    PoolGpuUnsupported,
    MissingSoloRpc,
    MissingPoolEndpoint,
}

pub fn resolve(cfg: &MinerConfig) -> Result<SidecarInvocation, ResolveError> {
    match (&cfg.mode, &cfg.device) {
        (Mode::Pool, Device::Gpu) => Err(ResolveError::PoolGpuUnsupported),
        (Mode::Pool, Device::Cpu) => {
            if cfg.pool_endpoint.trim().is_empty() {
                return Err(ResolveError::MissingPoolEndpoint);
            }
            let user = if cfg.worker_name.trim().is_empty() {
                cfg.address.clone()
            } else {
                format!("{}.{}", cfg.address, cfg.worker_name.trim())
            };
            let mut args = vec![
                "--stratum".into(),
                cfg.pool_endpoint.clone(),
                "--user".into(),
                user,
            ];
            if let Some(t) = cfg.threads {
                args.extend(["--threads".into(), t.to_string()]);
            }
            Ok(SidecarInvocation { program: "dinero-stratum-worker", args })
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
    fn base() -> MinerConfig {
        MinerConfig {
            address: "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700".into(),
            mode: Mode::Pool,
            device: Device::Cpu,
            threads: Some(4),
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
        assert_eq!(
            inv.args,
            vec![
                "--stratum",
                "pool.example.org:3333",
                "--user",
                "din1pafzgzwwfeqkfh7u4kkpe8qy97gey3zcvymx5eumxzx45m08q6tgqedz700.rig1",
                "--threads",
                "4",
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
    fn pool_gpu_is_a_typed_error() {
        let mut c = base();
        c.device = Device::Gpu;
        assert_eq!(resolve(&c), Err(ResolveError::PoolGpuUnsupported));
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
    #[test]
    fn empty_worker_name_uses_bare_address_as_user() {
        let mut c = base();
        c.worker_name = "".into();
        let inv = resolve(&c).unwrap();
        assert!(inv.args.contains(&c.address));
    }
}
