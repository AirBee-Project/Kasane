//! 両バックエンドで共通のバイト表現。
//!
//! シャードエントリの形式と値インデックスのキー組み立ては、どのストレージへ保存しても
//! 同じでよい（LMDB も TiKV もキーをバイト辞書順で並べる）。バックエンド実装が
//! 増えても表現が分岐しないよう、純粋な符号化だけをここへ集めている。
//!
//! バックエンド固有のキーレイアウトは各実装の `keys` モジュールにある。

pub mod shard_entry;
pub mod value_index;

/// キーへ埋め込む識別子のバイト長。
///
/// `DatabaseId` も `TableId` も中身は UUID なので、キーの中では同じ固定長を占める。
/// 固定長であること自体がキーレイアウトの前提で、これがあるおかげで
/// 「識別子 ‖ 可変長の続き」を曖昧さなく分解できる。
pub const UUID_LEN: usize = 16;

/// 与えたプレフィックスで始まる全キーを覆う範囲の終端（排他）。
///
/// 末尾のバイトを繰り上げて「次のプレフィックス」を作る。全バイトが 0xFF の場合は
/// 上限が存在しないので `None`（そのときは終端まで読めばよい）。
///
/// キーの並べ方はバックエンドごとに違うが、「バイト辞書順でプレフィックスを覆う」
/// という操作自体は共通なので、ここに 1 つだけ置く。
pub fn prefix_end(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end = prefix.to_vec();
    while let Some(last) = end.last_mut() {
        if *last < 0xFF {
            *last += 1;
            return Some(end);
        }
        end.pop();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_end_rolls_over_trailing_max_bytes() {
        assert_eq!(prefix_end(&[0x01, 0x02]), Some(vec![0x01, 0x03]));
        assert_eq!(prefix_end(&[0x01, 0xFF]), Some(vec![0x02]));
        assert_eq!(prefix_end(&[0xFF, 0xFF]), None);
        assert_eq!(prefix_end(&[]), None);
    }
}
