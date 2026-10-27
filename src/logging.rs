use tracing_subscriber::EnvFilter;

/// `RUST_LOG` が無ければ info 以上を出す。
pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
}
