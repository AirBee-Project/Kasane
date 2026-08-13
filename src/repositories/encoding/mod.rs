//! バックエンドに依存しない純粋な符号化。固有のキーレイアウトは各実装の `keys` にある。

pub mod shard_entry;
pub mod value_index;

/// キーへ埋め込む識別子のバイト長。
///
/// 固定長であることが「識別子 ‖ 可変長の続き」を曖昧さなく分解できる根拠になっている。
pub const UUID_LEN: usize = 16;

/// プレフィックスで始まる全キーを覆う範囲の終端（排他）。
/// 全バイトが 0xFF なら上限が存在しないので `None`。
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
