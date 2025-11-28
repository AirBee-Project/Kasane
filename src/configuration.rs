#[cfg(all(feature = "wasm", feature = "on-memory"))]
compile_error!("feature \"wasm\" and \"on-memory\" cannot be enabled at the same time.");

#[derive(Debug, Deserialize, Clone)]
pub struct Configuration {
    pub network: Network,
    pub general: General,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Network {
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct General {
    /// 指定されなかった場合は最大のCPU数を使用する
    pub cpu_num: Option<usize>,
    pub queue_size: usize,
    pub session_expiration_secs: u64,
}

#[cfg(feature = "wasm")]
#[derive(Debug, Clone)]
pub struct Configuration {}

#[cfg(feature = "on-memory")]
#[derive(Debug, Clone)]
pub struct Configuration {}
