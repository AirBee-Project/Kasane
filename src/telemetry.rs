use opentelemetry::global;
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace::Sampler};
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_telemetry() -> Option<opentelemetry_sdk::trace::SdkTracerProvider> {
    // W3C Trace Context の伝播を有効化（フロントエンドからの traceparent ヘッダーを引き継ぐ）
    global::set_text_map_propagator(TraceContextPropagator::new());

    // ログレベルのフィルタリング。デフォルトでアプリケーションとtower_http, OTelミドルウェアを出力対象にする
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,kasane=info,tower_http=info,axum_tracing_opentelemetry=info,otel::tracing=info",
        )
    });

    // OTLPエンドポイント（例: http://localhost:4317）が設定されていればトレーサーを有効化
    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    let (tracer, sdk_provider) = if let Some(endpoint) = otlp_endpoint {
        use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig, WithTonicConfig};

        let protocol = std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
            .unwrap_or_else(|_| "http/protobuf".to_string());

        let mut http_headers = std::collections::HashMap::new();
        if let Ok(header_str) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
            for kv in header_str.split(',') {
                let mut parts = kv.splitn(2, '=');
                if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                    let dec_k = percent_encoding::percent_decode_str(k.trim())
                        .decode_utf8_lossy()
                        .to_string();
                    let dec_v = percent_encoding::percent_decode_str(v.trim())
                        .decode_utf8_lossy()
                        .to_string();
                    http_headers.insert(dec_k, dec_v);
                }
            }
        }

        let exporter = if protocol == "grpc" {
            let mut metadata = tonic::metadata::MetadataMap::new();
            for (k, v) in &http_headers {
                let key_result = k
                    .to_lowercase()
                    .parse::<tonic::metadata::MetadataKey<tonic::metadata::Ascii>>();
                let val_result =
                    v.parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>();

                match (key_result, val_result) {
                    (Ok(key), Ok(val)) => {
                        metadata.insert(key, val);
                    }
                    (Err(e), _) => {
                        tracing::warn!("Failed to parse gRPC header key '{}': {}", k, e);
                    }
                    (_, Err(e)) => {
                        tracing::warn!("Failed to parse gRPC header value for key '{}': {}", k, e);
                    }
                }
            }
            SpanExporter::builder()
                .with_tonic()
                .with_endpoint(&endpoint)
                .with_metadata(metadata)
                .build()
                .expect("OTLP(gRPC) Exporterの構築に失敗しました")
        } else {
            SpanExporter::builder()
                .with_http()
                .with_endpoint(&endpoint)
                .with_headers(http_headers)
                .build()
                .expect("OTLP(HTTP) Exporterの構築に失敗しました")
        };

        let mut attributes = vec![opentelemetry::KeyValue::new(
            "service.namespace",
            "database",
        )];

        if let Ok(val) = std::env::var("CLOUD_REGION") {
            attributes.push(opentelemetry::KeyValue::new("cloud.region", val));
        }
        if let Ok(val) = std::env::var("CLOUD_AVAILABILITY_ZONE") {
            attributes.push(opentelemetry::KeyValue::new("cloud.availability_zone", val));
        }
        if let Ok(val) = std::env::var("CLOUD_PROVIDER") {
            attributes.push(opentelemetry::KeyValue::new("cloud.provider", val));
        }
        if let Ok(val) = std::env::var("DEPLOYMENT_ENVIRONMENT_NAME") {
            attributes.push(opentelemetry::KeyValue::new(
                "deployment.environment.name",
                val,
            ));
        }
        if let Ok(val) = std::env::var("HOST_NAME") {
            attributes.push(opentelemetry::KeyValue::new("host.name", val));
        }

        let resource = opentelemetry_sdk::Resource::builder_empty()
            .with_service_name("kasane")
            .with_attributes(attributes)
            .build();

        let sampler = if let Ok(ratio_str) = std::env::var("OTEL_TRACES_SAMPLER_ARG") {
            if let Ok(ratio) = ratio_str.parse::<f64>() {
                Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(ratio)))
            } else {
                tracing::warn!(
                    "不正な OTEL_TRACES_SAMPLER_ARG の値が設定されています: {}. 全件送信 (AlwaysOn) にフォールバックします。",
                    ratio_str
                );
                Sampler::AlwaysOn
            }
        } else {
            Sampler::AlwaysOn
        };

        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_sampler(sampler)
            .with_resource(resource)
            .build();

        global::set_tracer_provider(provider.clone());
        use opentelemetry::trace::TracerProvider;
        (Some(provider.tracer("kasane")), Some(provider))
    } else {
        (None, None)
    };

    // トレーサーがあればレイヤーに追加
    let telemetry_layer = tracer.map(|t| tracing_opentelemetry::layer().with_tracer(t));

    let log_format = std::env::var("KASANE_LOG_FORMAT").unwrap_or_else(|_| "plain".to_string());

    let registry = Registry::default().with(env_filter).with(telemetry_layer);

    // KASANE_LOG_FORMAT に応じてJSON出力とプレーンテキスト出力を切り替える
    if log_format == "json" {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }

    sdk_provider
}
