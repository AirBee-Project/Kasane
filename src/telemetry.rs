//! OpenTelemetry の初期化。トレース・メトリクス・ログの 3 信号を OTLP で送る。
//!
//! **設定は OTel の環境変数仕様に従う。** 独自の解釈を足すと、ベンダのドキュメント
//! どおりに設定したのに動かない、という事故になる（実際、以前はエンドポイントに
//! `/v1/traces` を自分で書く必要があった）。

use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "production")]
mod otel {
    use opentelemetry::{KeyValue, global};
    use opentelemetry_sdk::{
        Resource, logs::SdkLoggerProvider, metrics::SdkMeterProvider,
        propagation::TraceContextPropagator, trace::Sampler, trace::SdkTracerProvider,
    };

    /// 既定のサービス名。`OTEL_SERVICE_NAME` で上書きできる。
    const DEFAULT_SERVICE_NAME: &str = "kasane";

    /// 仕様以前から使っていたリソース属性の環境変数。
    ///
    /// `OTEL_RESOURCE_ATTRIBUTES` が正道だが、既存の配備が壊れないよう対応表として残す。
    const LEGACY_RESOURCE_VARS: &[(&str, &str)] = &[
        ("CLOUD_REGION", "cloud.region"),
        ("CLOUD_AVAILABILITY_ZONE", "cloud.availability_zone"),
        ("CLOUD_PROVIDER", "cloud.provider"),
        ("DEPLOYMENT_ENVIRONMENT_NAME", "deployment.environment.name"),
        ("HOST_NAME", "host.name"),
    ];

    fn env(name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.trim().is_empty())
    }

    /// `k=v,k=v` を分解する。値は仕様どおり percent-decode する。
    fn key_values(raw: &str) -> impl Iterator<Item = (String, String)> + '_ {
        raw.split(',').filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            let decode = |s: &str| {
                percent_encoding::percent_decode_str(s.trim())
                    .decode_utf8_lossy()
                    .into_owned()
            };
            Some((decode(k), decode(v)))
        })
    }

    /// OTLP のエンドポイントを**仕様どおり**に解決する。
    ///
    /// - `OTEL_EXPORTER_OTLP_<SIGNAL>_ENDPOINT` があればそのまま使う（パス込みが正）
    /// - 無ければ `OTEL_EXPORTER_OTLP_ENDPOINT` に `/v1/<signal>` を足す
    ///
    /// ベンダのドキュメントは前者を書かず `https://otlp.example.net` だけを示すことが
    /// 多い。パスを足さずに投げると 404 になるので、ここで吸収する。
    fn endpoint(signal: &str) -> Option<String> {
        let specific = format!("OTEL_EXPORTER_OTLP_{}_ENDPOINT", signal.to_uppercase());
        resolve_endpoint(env(&specific), env("OTEL_EXPORTER_OTLP_ENDPOINT"), signal)
    }

    /// 環境から切り離した本体。組み合わせが多いので単体で試験する。
    fn resolve_endpoint(
        specific: Option<String>,
        base: Option<String>,
        signal: &str,
    ) -> Option<String> {
        if let Some(url) = specific {
            return Some(url);
        }
        let base = base?;
        let base = base.trim_end_matches('/');
        // 既に信号のパスが付いていれば二重に足さない（従来の設定との互換）。
        if base.ends_with(&format!("/v1/{signal}")) {
            return Some(base.to_string());
        }
        Some(format!("{base}/v1/{signal}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn resolve(specific: Option<&str>, base: Option<&str>) -> Option<String> {
            resolve_endpoint(
                specific.map(str::to_string),
                base.map(str::to_string),
                "traces",
            )
        }

        /// ベンダのドキュメントどおり（パス無し）でも信号のパスが付くこと。
        #[test]
        fn generic_endpoint_gains_the_signal_path() {
            assert_eq!(
                resolve(None, Some("https://otlp.example.net")).as_deref(),
                Some("https://otlp.example.net/v1/traces")
            );
            assert_eq!(
                resolve(None, Some("https://otlp.example.net/")).as_deref(),
                Some("https://otlp.example.net/v1/traces")
            );
        }

        /// 既にパスを書いてある従来の設定を壊さないこと。
        #[test]
        fn an_explicit_signal_path_is_not_doubled() {
            assert_eq!(
                resolve(None, Some("https://otlp.example.net/v1/traces")).as_deref(),
                Some("https://otlp.example.net/v1/traces")
            );
        }

        /// 信号別の変数はそのまま使う（パスを足さない）。
        #[test]
        fn the_signal_specific_variable_wins_verbatim() {
            assert_eq!(
                resolve(Some("https://a.example/custom"), Some("https://b.example")).as_deref(),
                Some("https://a.example/custom")
            );
        }

        #[test]
        fn nothing_configured_means_no_export() {
            assert_eq!(resolve(None, None), None);
        }

        #[test]
        fn headers_and_attributes_are_percent_decoded() {
            let kv: Vec<_> = key_values("api%2Dkey=abc%20123,plain=v").collect();
            assert_eq!(
                kv,
                vec![
                    ("api-key".to_string(), "abc 123".to_string()),
                    ("plain".to_string(), "v".to_string())
                ]
            );
        }
    }

    fn headers() -> std::collections::HashMap<String, String> {
        env("OTEL_EXPORTER_OTLP_HEADERS")
            .map(|raw| key_values(&raw).collect())
            .unwrap_or_default()
    }

    fn resource() -> Resource {
        let mut attributes: Vec<KeyValue> = vec![
            KeyValue::new("service.namespace", "database"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ];

        if let Some(raw) = env("OTEL_RESOURCE_ATTRIBUTES") {
            attributes.extend(key_values(&raw).map(|(k, v)| KeyValue::new(k, v)));
        }
        for (var, key) in LEGACY_RESOURCE_VARS {
            if let Some(value) = env(var) {
                attributes.push(KeyValue::new(*key, value));
            }
        }

        Resource::builder_empty()
            .with_service_name(
                env("OTEL_SERVICE_NAME").unwrap_or_else(|| DEFAULT_SERVICE_NAME.into()),
            )
            .with_attributes(attributes)
            .build()
    }

    /// `OTEL_TRACES_SAMPLER` / `OTEL_TRACES_SAMPLER_ARG` を仕様どおりに解釈する。
    ///
    /// 既定は全件収集。取りこぼしを減らすことを優先し、間引きは明示設定に任せる。
    fn sampler() -> Sampler {
        let ratio = || {
            env("OTEL_TRACES_SAMPLER_ARG")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(1.0)
        };
        match env("OTEL_TRACES_SAMPLER").as_deref() {
            // 仕様上の既定値。
            None | Some("parentbased_always_on") => {
                Sampler::ParentBased(Box::new(Sampler::AlwaysOn))
            }
            Some("always_on") => Sampler::AlwaysOn,
            Some("always_off") => Sampler::AlwaysOff,
            Some("traceidratio") => Sampler::TraceIdRatioBased(ratio()),
            Some("parentbased_always_off") => Sampler::ParentBased(Box::new(Sampler::AlwaysOff)),
            Some("parentbased_traceidratio") => {
                Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(ratio())))
            }
            Some(other) => {
                tracing::warn!(
                    "unknown OTEL_TRACES_SAMPLER value '{other}'; falling back to always_on"
                );
                Sampler::AlwaysOn
            }
        }
    }

    /// 立ち上げた各プロバイダ。終了時にまとめて flush する。
    #[derive(Default)]
    pub struct Providers {
        pub tracer: Option<SdkTracerProvider>,
        pub meter: Option<SdkMeterProvider>,
        pub logger: Option<SdkLoggerProvider>,
    }

    impl Providers {
        /// **バッファに残ったぶんを送り切る。** 短命なプロセスではここを通さないと
        /// 最後のリクエストが丸ごと消える。
        pub fn shutdown(&self) {
            if let Some(p) = &self.tracer
                && let Err(e) = p.shutdown()
            {
                eprintln!("failed to shut down the tracer provider: {e:?}");
            }
            if let Some(p) = &self.meter
                && let Err(e) = p.shutdown()
            {
                eprintln!("failed to shut down the meter provider: {e:?}");
            }
            if let Some(p) = &self.logger
                && let Err(e) = p.shutdown()
            {
                eprintln!("failed to shut down the logger provider: {e:?}");
            }
        }
    }

    pub fn tracer_provider() -> Option<SdkTracerProvider> {
        use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};

        let exporter = SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint("traces")?)
            .with_headers(headers())
            .build()
            .expect("failed to build the OTLP (HTTP) span exporter");

        let processor = opentelemetry_sdk::trace::span_processor_with_async_runtime::
            BatchSpanProcessor::builder(exporter, opentelemetry_sdk::runtime::Tokio).build();

        let provider = SdkTracerProvider::builder()
            .with_span_processor(processor)
            .with_sampler(sampler())
            .with_resource(resource())
            .build();

        global::set_tracer_provider(provider.clone());
        Some(provider)
    }

    /// **`Tokio` 束縛の周期リーダーを使うこと。**
    ///
    /// 既定の `PeriodicReader` は専用の OS スレッドでエクスポートを回すが、この
    /// バイナリの OTLP(HTTP) エクスポータ（`reqwest`）は非同期処理に Tokio のリアクタを
    /// 要求する。Tokio の外側で呼ぶと "there is no reactor running" で panic する
    /// （実際に踏んだ： 60 秒おきのエクスポートがバックグラウンドスレッドの上で
    /// クラッシュし、その周期以降メトリクスが一切送られなくなっていた）。トレースの
    /// `BatchSpanProcessor` と同じ理由で同じ対処をする。
    pub fn meter_provider() -> Option<SdkMeterProvider> {
        use opentelemetry_otlp::{MetricExporter, WithExportConfig, WithHttpConfig};
        use opentelemetry_sdk::metrics::periodic_reader_with_async_runtime::PeriodicReader;

        let exporter = MetricExporter::builder()
            .with_http()
            .with_endpoint(endpoint("metrics")?)
            .with_headers(headers())
            .build()
            .expect("failed to build the OTLP (HTTP) metric exporter");

        let reader = PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio).build();
        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(resource())
            .build();

        global::set_meter_provider(provider.clone());
        Some(provider)
    }

    /// [`meter_provider`] と同じ理由で `Tokio` 束縛の [`BatchLogProcessor`] を使う。
    pub fn logger_provider() -> Option<SdkLoggerProvider> {
        use opentelemetry_otlp::{LogExporter, WithExportConfig, WithHttpConfig};
        use opentelemetry_sdk::logs::log_processor_with_async_runtime::BatchLogProcessor;

        let exporter = LogExporter::builder()
            .with_http()
            .with_endpoint(endpoint("logs")?)
            .with_headers(headers())
            .build()
            .expect("failed to build the OTLP (HTTP) log exporter");

        let processor =
            BatchLogProcessor::builder(exporter, opentelemetry_sdk::runtime::Tokio).build();

        Some(
            SdkLoggerProvider::builder()
                .with_log_processor(processor)
                .with_resource(resource())
                .build(),
        )
    }

    pub fn set_propagator() {
        global::set_text_map_propagator(TraceContextPropagator::new());
    }
}

#[cfg(feature = "production")]
pub use otel::Providers;

/// `production` 以外では何も持たない。呼び出し側の分岐を無くすためだけに置く。
#[cfg(not(feature = "production"))]
#[derive(Default)]
pub struct Providers;

#[cfg(not(feature = "production"))]
impl Providers {
    pub fn shutdown(&self) {}
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,kasane=info,tower_http=info,axum_tracing_opentelemetry=error,otel::tracing=info",
        )
    })
}

fn json_logs() -> bool {
    std::env::var("KASANE_LOG_FORMAT").is_ok_and(|v| v == "json")
}

#[cfg(feature = "production")]
pub fn init_telemetry() -> Providers {
    otel::set_propagator();

    let providers = Providers {
        tracer: otel::tracer_provider(),
        meter: otel::meter_provider(),
        logger: otel::logger_provider(),
    };

    let trace_layer = providers.tracer.as_ref().map(|p| {
        use opentelemetry::trace::TracerProvider;
        tracing_opentelemetry::layer().with_tracer(p.tracer("kasane"))
    });

    // tracing のイベントを OTLP ログとして送る。スパン文脈が自動で載るので、
    // New Relic 側でトレースとログが相互に辿れる。
    let log_layer = providers
        .logger
        .as_ref()
        .map(opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new);

    let registry = Registry::default()
        .with(env_filter())
        .with(trace_layer)
        .with(log_layer);

    if json_logs() {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }

    providers
}

#[cfg(not(feature = "production"))]
pub fn init_telemetry() -> Providers {
    let registry = Registry::default().with(env_filter());

    if json_logs() {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }

    Providers
}

/// アプリ固有の計器。
///
/// **スパンから導けない量だけを置く。** リクエスト数やレイテンシはスパンからも読めるが、
/// 間引き設定を入れた途端に不正確になるので、レートは計器側でも持つ。再試行回数や
/// 回収件数はそもそもスパンに現れない。
///
/// `production` 以外では全て空関数になる（呼び出し側に `cfg` を撒かないため）。
pub mod metrics {
    /// 書き込みトランザクションの結末。再試行の原因を分けて数えるためにある。
    #[derive(Debug, Clone, Copy)]
    pub enum WriteOutcome {
        Committed,
        /// 競合で捨ててやり直した。
        Conflict,
        /// ロックが足りず宣言し直した（TiKV のみ）。
        LockDeclared,
        /// 読んだ木の形が古かった（TiKV のみ）。
        Stale,
        Failed,
    }

    impl WriteOutcome {
        /// `production` の計器だけが読む。それ以外のビルドでは `imp` が中身を捨てる
        /// no-op になるので、この変換自体が要らない。
        #[cfg(feature = "production")]
        pub(super) const fn as_str(self) -> &'static str {
            match self {
                Self::Committed => "committed",
                Self::Conflict => "conflict",
                Self::LockDeclared => "lock_declared",
                Self::Stale => "stale",
                Self::Failed => "failed",
            }
        }
    }

    #[cfg(feature = "production")]
    mod imp {
        use super::WriteOutcome;
        use opentelemetry::metrics::{Counter, Histogram};
        use opentelemetry::{KeyValue, global};
        use std::sync::OnceLock;

        struct Instruments {
            http_duration: Histogram<f64>,
            write_attempts: Counter<u64>,
            write_duration: Histogram<f64>,
            read_duration: Histogram<f64>,
            batch_size: Histogram<u64>,
            gc_reclaimed: Counter<u64>,
        }

        fn instruments() -> &'static Instruments {
            static I: OnceLock<Instruments> = OnceLock::new();
            I.get_or_init(|| {
                let m = global::meter("kasane");
                Instruments {
                    http_duration: m
                        .f64_histogram("http.server.request.duration")
                        .with_unit("s")
                        .with_description("Duration of inbound HTTP requests.")
                        .build(),
                    write_attempts: m
                        .u64_counter("kasane.storage.write.attempts")
                        .with_description("Write transaction attempts, split by outcome.")
                        .build(),
                    write_duration: m
                        .f64_histogram("kasane.storage.write.duration")
                        .with_unit("s")
                        .with_description("Duration of a write transaction, retries included.")
                        .build(),
                    read_duration: m
                        .f64_histogram("kasane.storage.read.duration")
                        .with_unit("s")
                        .with_description("Duration of a read transaction.")
                        .build(),
                    batch_size: m
                        .u64_histogram("kasane.write.batch.size")
                        .with_description("Requests coalesced into one write transaction.")
                        .build(),
                    gc_reclaimed: m
                        .u64_counter("kasane.gc.reclaimed")
                        .with_description("Keys reclaimed by the background sweeper.")
                        .build(),
                }
            })
        }

        pub fn http_request(method: &str, route: String, status: u16, seconds: f64) {
            instruments().http_duration.record(
                seconds,
                &[
                    KeyValue::new("http.request.method", method.to_string()),
                    KeyValue::new("http.route", route),
                    KeyValue::new("http.response.status_code", status as i64),
                ],
            );
        }

        pub fn write_attempt(outcome: WriteOutcome) {
            instruments().write_attempts.add(
                1,
                &[
                    KeyValue::new("backend", crate::backend::NAME),
                    KeyValue::new("outcome", outcome.as_str()),
                ],
            );
        }

        pub fn write_transaction(seconds: f64) {
            instruments()
                .write_duration
                .record(seconds, &[KeyValue::new("backend", crate::backend::NAME)]);
        }

        pub fn read_transaction(seconds: f64) {
            instruments()
                .read_duration
                .record(seconds, &[KeyValue::new("backend", crate::backend::NAME)]);
        }

        pub fn write_batch(size: usize) {
            instruments().batch_size.record(size as u64, &[]);
        }

        pub fn gc_reclaimed(keys: usize) {
            instruments().gc_reclaimed.add(keys as u64, &[]);
        }
    }

    #[cfg(not(feature = "production"))]
    mod imp {
        use super::WriteOutcome;
        pub fn http_request(_: &str, _: String, _: u16, _: f64) {}
        pub fn write_attempt(_: WriteOutcome) {}
        pub fn write_transaction(_: f64) {}
        pub fn read_transaction(_: f64) {}
        pub fn write_batch(_: usize) {}
        pub fn gc_reclaimed(_: usize) {}
    }

    pub use imp::{
        gc_reclaimed, http_request, read_transaction, write_attempt, write_batch, write_transaction,
    };
}
