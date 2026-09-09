use crate::client::ProxyHttpClient;
use crate::grpc_frame::{decode_all_grpc_frames, decode_grpc_frame, encode_grpc_frame};
use crate::merger::merge_query_streams;
use crate::partition::{SubQueryTask, split_query};
use crate::types::{BoxStream, ProxyRequest, ProxyResponse, ProxyResponseBody};
use futures::StreamExt;
use http::{HeaderMap, StatusCode};
use kasane_proto::kasane::{ExecuteQueryRequest, SearchDataResponse};

/// クエリ実行の gRPC パス定数
pub const QUERY_EXECUTE_PATH: &str = "/kasane.QueryService/Execute";

/// プロキシゲートウェイの設定
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// データ元となる Origin Kasane のベース URL (例: "http://127.0.0.1:5172")
    pub origin_url: String,
    /// クエリ計算を分散させる Worker のベース URL リスト
    pub query_worker_urls: Vec<String>,
}

impl ProxyConfig {
    pub fn new(origin_url: impl Into<String>) -> Self {
        let mut origin_url = origin_url.into();
        while origin_url.ends_with('/') {
            origin_url.pop();
        }
        Self {
            origin_url,
            query_worker_urls: Vec::new(),
        }
    }

    /// クエリ分散用 Worker の URL リストを設定
    pub fn with_query_worker_urls(mut self, urls: Vec<String>) -> Self {
        self.query_worker_urls = urls
            .into_iter()
            .map(|mut u| {
                while u.ends_with('/') {
                    u.pop();
                }
                u
            })
            .filter(|u| !u.is_empty())
            .collect();
        self
    }
}

/// 全プラットフォーム共通のプロキシエントリーポイント
///
/// すべての着信リクエストを受け付け、適切な処理先へ振り分ける。
/// - クエリ実行かつ Worker URLs が設定されている場合: クエリ分割 -> 並行Worker送信 -> ストリーム合流
/// - それ以外: Origin Kasane へ透過転送
pub async fn handle_proxy_request<C: ProxyHttpClient>(
    client: &C,
    config: &ProxyConfig,
    request: ProxyRequest,
) -> Result<ProxyResponse, C::Error> {
    if is_query_execute_request(&request) && !config.query_worker_urls.is_empty() {
        tracing::debug!(
            workers = config.query_worker_urls.len(),
            "Handling partitioned query execution request"
        );
        handle_partitioned_query(client, config, request).await
    } else {
        // [Transparent Forwarding]
        // CRUD, 認証, メタデータ, システム等の全リクエストを透過フォワード
        tracing::debug!(
            path = request.path(),
            "Transparently forwarding request to origin"
        );
        client.forward(&config.origin_url, request).await
    }
}

/// クエリ実行リクエストかどうかを判定する
pub fn is_query_execute_request(request: &ProxyRequest) -> bool {
    request.path() == QUERY_EXECUTE_PATH
}

/// クエリを分割し、複数 Worker に並行送信して結果ストリームを合流させる
async fn handle_partitioned_query<C: ProxyHttpClient>(
    client: &C,
    config: &ProxyConfig,
    request: ProxyRequest,
) -> Result<ProxyResponse, C::Error> {
    // 1. gRPC フレームから ExecuteQueryRequest をデコード
    let query_req = match decode_grpc_frame::<ExecuteQueryRequest>(&request.body) {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("Failed to decode ExecuteQueryRequest gRPC frame: {e}");
            return Ok(ProxyResponse::new(
                StatusCode::BAD_REQUEST,
                HeaderMap::new(),
                bytes::Bytes::from(format!("Invalid ExecuteQueryRequest gRPC frame: {e}")),
            ));
        }
    };

    // 2. split_query (TODO) を呼び出してクエリを分割
    let sub_tasks = match split_query(&query_req, &config.query_worker_urls) {
        Ok(tasks) => tasks,
        Err(e) => {
            tracing::error!("Failed to partition query: {e}");
            return Ok(ProxyResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                HeaderMap::new(),
                bytes::Bytes::from(format!("Query partition error: {e}")),
            ));
        }
    };

    execute_subtasks_and_merge(client, &request, sub_tasks).await
}

/// サブクエリ群を並行送信し、レスポンスストリームを合流するコアロジック
pub async fn execute_subtasks_and_merge<C: ProxyHttpClient>(
    client: &C,
    original_request: &ProxyRequest,
    sub_tasks: Vec<SubQueryTask>,
) -> Result<ProxyResponse, C::Error> {
    let mut worker_streams = Vec::with_capacity(sub_tasks.len());

    // 各 Worker へ投げる Future を構築
    let futures: Vec<_> = sub_tasks
        .into_iter()
        .map(|task| {
            let client = client;
            let sub_body = encode_grpc_frame(&task.request)
                .expect("Valid protobuf ExecuteQueryRequest encoding");
            let sub_req = ProxyRequest::new(
                original_request.method.clone(),
                original_request.uri.clone(),
                original_request.headers.clone(),
                sub_body,
            );
            async move { client.forward(&task.target_url, sub_req).await }
        })
        .collect();

    // 並行送信 (Fan-out)
    let results = futures::future::join_all(futures).await;

    for res in results {
        match res {
            Ok(proxy_res) => {
                let stream = response_to_search_data_stream(proxy_res);
                worker_streams.push(stream);
            }
            Err(e) => {
                tracing::error!("Worker request failed: {e}");
                return Err(e);
            }
        }
    }

    // ストリーム合流 (Fan-in) & 辞書リマップ
    let merged_stream = merge_query_streams(worker_streams);

    // gRPC レスポンスヘッダを構成
    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/grpc"),
    );

    Ok(ProxyResponse::from_stream(
        StatusCode::OK,
        headers,
        Box::pin(merged_stream),
    ))
}

/// `ProxyResponse` のボディを一貫して `Stream<Item = Result<SearchDataResponse, String>>` に変換
fn response_to_search_data_stream(
    resp: ProxyResponse,
) -> BoxStream<Result<SearchDataResponse, String>> {
    match resp.body {
        ProxyResponseBody::Bytes(bytes) => {
            let res = decode_all_grpc_frames::<SearchDataResponse>(&bytes)
                .map_err(|e| format!("Failed to decode gRPC frames from worker: {e}"));
            match res {
                Ok(frames) => Box::pin(futures::stream::iter(frames.into_iter().map(Ok))),
                Err(e) => Box::pin(futures::stream::once(async move { Err(e) })),
            }
        }
        ProxyResponseBody::Stream(mut byte_stream) => Box::pin(async_stream::stream! {
            let mut buffer = bytes::BytesMut::new();
            while let Some(chunk_res) = byte_stream.next().await {
                match chunk_res {
                    Ok(chunk) => {
                        buffer.extend_from_slice(&chunk);
                        while buffer.len() >= crate::grpc_frame::GRPC_HEADER_SIZE {
                            let msg_len = u32::from_be_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]) as usize;
                            let total_len = crate::grpc_frame::GRPC_HEADER_SIZE + msg_len;
                            if buffer.len() >= total_len {
                                let frame_bytes = buffer.split_to(total_len);
                                match decode_grpc_frame::<SearchDataResponse>(&frame_bytes) {
                                    Ok(msg) => yield Ok(msg),
                                    Err(e) => {
                                        yield Err(format!("gRPC frame decode error: {e}"));
                                        return;
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                }
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http::{HeaderMap, Method, StatusCode, Uri};
    use kasane_proto::kasane::{DataGroup, SpatialId, TypedValue, data_group, typed_value::Kind};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct MockHttpClient {
        forward_call_count: Arc<AtomicUsize>,
        urls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait(?Send)]
    impl ProxyHttpClient for MockHttpClient {
        type Error = String;

        async fn forward(
            &self,
            origin_base_url: &str,
            _request: ProxyRequest,
        ) -> Result<ProxyResponse, Self::Error> {
            self.forward_call_count.fetch_add(1, Ordering::SeqCst);
            self.urls.lock().unwrap().push(origin_base_url.to_string());

            let resp = SearchDataResponse {
                dictionary: vec![TypedValue {
                    kind: Some(Kind::StringVal(format!("val-from-{}", origin_base_url))),
                }],
                data: vec![DataGroup {
                    value: Some(data_group::Value::DictRef(0)),
                    spatial_ids: vec![SpatialId::default()],
                }],
            };
            let frame = encode_grpc_frame(&resp).unwrap();
            Ok(ProxyResponse::from_bytes(
                StatusCode::OK,
                HeaderMap::new(),
                frame,
            ))
        }
    }

    #[tokio::test]
    async fn test_handle_transparent_forwarding_when_no_workers() {
        let forward_call_count = Arc::new(AtomicUsize::new(0));
        let urls = Arc::new(Mutex::new(Vec::new()));
        let client = MockHttpClient {
            forward_call_count: forward_call_count.clone(),
            urls: urls.clone(),
        };

        let config = ProxyConfig::new("http://origin-kasane:5172");
        let request = ProxyRequest::new(
            Method::POST,
            Uri::from_static("/kasane.TableService/Create"),
            HeaderMap::new(),
            Bytes::from_static(b"payload"),
        );

        let response = handle_proxy_request(&client, &config, request)
            .await
            .unwrap();

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(forward_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            urls.lock().unwrap().as_slice(),
            &["http://origin-kasane:5172".to_string()]
        );
    }

    #[tokio::test]
    async fn test_subtasks_fanout_and_stream_merge() {
        let forward_call_count = Arc::new(AtomicUsize::new(0));
        let urls = Arc::new(Mutex::new(Vec::new()));
        let client = MockHttpClient {
            forward_call_count: forward_call_count.clone(),
            urls: urls.clone(),
        };

        let orig_req = ProxyRequest::new(
            Method::POST,
            Uri::from_static(QUERY_EXECUTE_PATH),
            HeaderMap::new(),
            Bytes::new(),
        );

        let sub_tasks = vec![
            SubQueryTask {
                target_url: "http://worker-1:5172".to_string(),
                request: ExecuteQueryRequest::default(),
            },
            SubQueryTask {
                target_url: "http://worker-2:5172".to_string(),
                request: ExecuteQueryRequest::default(),
            },
        ];

        let response = execute_subtasks_and_merge(&client, &orig_req, sub_tasks)
            .await
            .unwrap();

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(forward_call_count.load(Ordering::SeqCst), 2);

        // 合流ストリームからチャンクを取り出して検証
        if let ProxyResponseBody::Stream(mut stream) = response.body {
            let mut merged_messages = Vec::new();
            while let Some(chunk_res) = stream.next().await {
                let frame_bytes = chunk_res.unwrap();
                let decoded: SearchDataResponse = decode_grpc_frame(&frame_bytes).unwrap();
                merged_messages.push(decoded);
            }
            assert_eq!(merged_messages.len(), 2);
        } else {
            panic!("Expected streaming response body");
        }
    }
}
