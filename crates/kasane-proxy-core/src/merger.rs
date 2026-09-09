use crate::grpc_frame::encode_grpc_frame;
use bytes::Bytes;
use futures::Stream;
use kasane_proto::kasane::{
    DataGroup, SearchDataResponse, TypedValue, data_group, typed_value::Kind,
};
use std::collections::HashMap;

/// `TypedValue` を `HashMap` のキーとして扱うためのハッシュ可能ラッパー
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HashableTypedValue {
    StringVal(String),
    IntVal(i64),
    BoolVal(bool),
    NullVal(i32),
    None,
}

impl From<&TypedValue> for HashableTypedValue {
    fn from(tv: &TypedValue) -> Self {
        match &tv.kind {
            Some(Kind::StringVal(s)) => Self::StringVal(s.clone()),
            Some(Kind::IntVal(i)) => Self::IntVal(*i),
            Some(Kind::BoolVal(b)) => Self::BoolVal(*b),
            Some(Kind::NullVal(n)) => Self::NullVal(*n),
            None => Self::None,
        }
    }
}

/// 複数 Worker から返ってくる `SearchDataResponse` の辞書インデックスを
/// グローバルな一貫性を持つ単一辞書ストリームへリマップ・合流するマージャー
pub struct StreamMerger {
    /// 各 Worker ごとのローカル累積辞書
    worker_dicts: HashMap<usize, Vec<TypedValue>>,
    /// グローバル辞書テーブル
    global_dict: Vec<TypedValue>,
    /// グローバル辞書の逆引きマップ (値 -> グローバル dict_ref)
    global_dict_map: HashMap<HashableTypedValue, u64>,
}

impl StreamMerger {
    pub fn new() -> Self {
        Self {
            worker_dicts: HashMap::new(),
            global_dict: Vec::new(),
            global_dict_map: HashMap::new(),
        }
    }

    /// 特定の Worker から届いた 1 チャンクの `SearchDataResponse` を処理し、
    /// グローバル辞書にリマップされた合流用 `SearchDataResponse` を生成する。
    pub fn process_chunk(
        &mut self,
        worker_id: usize,
        incoming: SearchDataResponse,
    ) -> SearchDataResponse {
        // 1. Worker のローカル辞書を更新
        let local_dict = self.worker_dicts.entry(worker_id).or_default();
        local_dict.extend(incoming.dictionary);

        // 2. DataGroup 内の辞書インデックスをグローバル辞書へリマップ
        let mut new_dict_entries = Vec::new();
        let mut remapped_data = Vec::with_capacity(incoming.data.len());

        let local_dict = &self.worker_dicts[&worker_id];
        let global_dict = &mut self.global_dict;
        let global_dict_map = &mut self.global_dict_map;

        for group in incoming.data {
            let remapped_value = match group.value {
                Some(data_group::Value::DictRef(local_ref)) => {
                    // ローカル辞書から実値を取得
                    if let Some(val) = local_dict.get(local_ref as usize) {
                        let global_ref = Self::get_or_insert_global_entry(
                            global_dict,
                            global_dict_map,
                            val,
                            &mut new_dict_entries,
                        );
                        Some(data_group::Value::DictRef(global_ref))
                    } else {
                        // 辞書に存在しない不正参照はそのまま維持
                        Some(data_group::Value::DictRef(local_ref))
                    }
                }
                Some(data_group::Value::InlineValue(val)) => {
                    // インライン値もグローバル辞書に登録して辞書参照化
                    let global_ref = Self::get_or_insert_global_entry(
                        global_dict,
                        global_dict_map,
                        &val,
                        &mut new_dict_entries,
                    );
                    Some(data_group::Value::DictRef(global_ref))
                }
                None => None,
            };

            remapped_data.push(DataGroup {
                value: remapped_value,
                spatial_ids: group.spatial_ids,
            });
        }

        SearchDataResponse {
            dictionary: new_dict_entries,
            data: remapped_data,
        }
    }

    /// 値をグローバル辞書に登録し、その global dict_ref を返す。
    /// 未登録だった場合は `new_entries` に追加される。
    fn get_or_insert_global_entry(
        global_dict: &mut Vec<TypedValue>,
        global_dict_map: &mut HashMap<HashableTypedValue, u64>,
        val: &TypedValue,
        new_entries: &mut Vec<TypedValue>,
    ) -> u64 {
        let key = HashableTypedValue::from(val);
        if let Some(&existing_idx) = global_dict_map.get(&key) {
            existing_idx
        } else {
            let new_idx = global_dict.len() as u64;
            global_dict.push(val.clone());
            global_dict_map.insert(key, new_idx);
            new_entries.push(val.clone());
            new_idx
        }
    }

    /// 現在のグローバル辞書全体の長さを取得
    pub fn global_dict_len(&self) -> usize {
        self.global_dict.len()
    }
}

impl Default for StreamMerger {
    fn default() -> Self {
        Self::new()
    }
}

/// 複数 Worker からの `SearchDataResponse` ストリーム群を辞書リマップし、
/// 単一の gRPC フレームバイト列ストリームとして合流させる
pub fn merge_query_streams<S>(
    worker_streams: Vec<S>,
) -> impl Stream<Item = Result<Bytes, String>>
where
    S: Stream<Item = Result<SearchDataResponse, String>> + Unpin + 'static,
{
    use futures::StreamExt;

    // 各ストリームに Worker ID をタグ付け
    let tagged_streams = worker_streams
        .into_iter()
        .enumerate()
        .map(|(worker_id, s)| s.map(move |item| (worker_id, item)));

    let mut select_all = futures::stream::select_all(tagged_streams);
    let mut merger = StreamMerger::new();

    async_stream::stream! {
        while let Some((worker_id, item)) = select_all.next().await {
            match item {
                Ok(resp) => {
                    let remapped = merger.process_chunk(worker_id, resp);
                    match encode_grpc_frame(&remapped) {
                        Ok(frame) => yield Ok(frame),
                        Err(e) => yield Err(format!("gRPC encode error in merger: {e}")),
                    }
                }
                Err(e) => {
                    yield Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kasane_proto::kasane::SpatialId;

    #[test]
    fn test_dictionary_remapping() {
        let mut merger = StreamMerger::new();

        // Worker 0: dictionary = ["apple", "banana"]
        let resp0 = SearchDataResponse {
            dictionary: vec![
                TypedValue {
                    kind: Some(Kind::StringVal("apple".to_string())),
                },
                TypedValue {
                    kind: Some(Kind::StringVal("banana".to_string())),
                },
            ],
            data: vec![
                DataGroup {
                    value: Some(data_group::Value::DictRef(0)), // "apple"
                    spatial_ids: vec![SpatialId::default()],
                },
                DataGroup {
                    value: Some(data_group::Value::DictRef(1)), // "banana"
                    spatial_ids: vec![SpatialId::default()],
                },
            ],
        };

        // Worker 1: dictionary = ["orange", "apple"]
        // Worker 1 の 0 は "orange", 1 は "apple"
        let resp1 = SearchDataResponse {
            dictionary: vec![
                TypedValue {
                    kind: Some(Kind::StringVal("orange".to_string())),
                },
                TypedValue {
                    kind: Some(Kind::StringVal("apple".to_string())),
                },
            ],
            data: vec![
                DataGroup {
                    value: Some(data_group::Value::DictRef(0)), // "orange"
                    spatial_ids: vec![SpatialId::default()],
                },
                DataGroup {
                    value: Some(data_group::Value::DictRef(1)), // "apple"
                    spatial_ids: vec![SpatialId::default()],
                },
            ],
        };

        let mapped0 = merger.process_chunk(0, resp0);
        // mapped0 では apple(0), banana(1) が新辞書に追加される
        assert_eq!(mapped0.dictionary.len(), 2);
        assert_eq!(mapped0.data[0].value, Some(data_group::Value::DictRef(0)));
        assert_eq!(mapped0.data[1].value, Some(data_group::Value::DictRef(1)));

        let mapped1 = merger.process_chunk(1, resp1);
        // mapped1 では orange のみが新辞書に追加される (apple は既に 0 として存在)
        assert_eq!(mapped1.dictionary.len(), 1);
        assert_eq!(
            mapped1.dictionary[0].kind,
            Some(Kind::StringVal("orange".to_string()))
        );
        // orange は global 2, apple は global 0
        assert_eq!(mapped1.data[0].value, Some(data_group::Value::DictRef(2)));
        assert_eq!(mapped1.data[1].value, Some(data_group::Value::DictRef(0)));

        assert_eq!(merger.global_dict_len(), 3);
    }
}
