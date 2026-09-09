use kasane_proto::kasane::ExecuteQueryRequest;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueryPartitionError {
    #[error("No target workers available")]
    NoWorkersAvailable,
    #[error("Query cannot be partitioned: {0}")]
    Unpartitionable(String),
    #[error("Protobuf codec error: {0}")]
    CodecError(String),
}

/// 各 Worker へ割り振るサブクエリタスク
#[derive(Debug, Clone, PartialEq)]
pub struct SubQueryTask {
    /// 割当先 Worker のベース URL (例: "https://worker-1.kasane.internal")
    pub target_url: String,
    /// その Worker に実行させる分割クエリリクエスト
    pub request: ExecuteQueryRequest,
}

/// クエリ分割関数のエントリーポイント
///
/// 元のリクエストと Worker の URL リストを受け取り、各 Worker に分配するサブクエリ群を生成する。
///
/// # Note
/// 現在は TODO として定義されており、クエリ分割ロジックは後続フェーズで実装されます。
pub fn split_query(
    request: &ExecuteQueryRequest,
    target_workers: &[String],
) -> Result<Vec<SubQueryTask>, QueryPartitionError> {
    let _ = (request, target_workers);
    // TODO: クエリ分割ロジックを実装する（この後に書く）
    todo!("query partitioning logic will be implemented later")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "query partitioning logic will be implemented later")]
    fn test_split_query_todo() {
        let req = ExecuteQueryRequest::default();
        let workers = vec!["http://worker-1:5172".to_string()];
        let _ = split_query(&req, &workers);
    }
}
