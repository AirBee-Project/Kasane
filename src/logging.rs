use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_FILTER: &str = "info,kasane=info,spatial_id=debug,tower_http=info";

/// `RUST_LOG` の値をそのまま使わず、spatial-id の詳細トレースを常時追記する。
/// デプロイ環境の `RUST_LOG` を変更せずに効かせるため。
fn env_filter() -> EnvFilter {
    match std::env::var("RUST_LOG") {
        Ok(value) if !value.trim().is_empty() => {
            EnvFilter::new(format!("{value},spatial_id=debug"))
        }
        _ => EnvFilter::new(DEFAULT_FILTER),
    }
}

pub fn init() {
    let registry = Registry::default().with(env_filter());

    if std::env::var("KASANE_LOG_FORMAT").is_ok_and(|v| v == "json") {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
}
