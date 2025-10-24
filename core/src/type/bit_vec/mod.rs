use std::{
    cmp::Ordering,
    fmt,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord)]
pub struct BitVec(pub(crate) Vec<u8>);

impl fmt::Display for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_bits = self.0.len() * 8;
        for i in 0..total_bits {
            write!(f, "{}", self.get_bit(i))?;
        }
        Ok(())
    }
}

impl PartialOrd for BitVec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl BitVec {
    /// Vec<u8> から BitVec を生成
    pub fn from_vec(v: Vec<u8>) -> Self {
        BitVec(v)
    }

    /// スライスから BitVec を生成
    pub fn from_slice(s: &[u8]) -> Self {
        BitVec(s.to_vec())
    }

    /// 空の BitVec を生成
    pub fn new() -> Self {
        BitVec(Vec::new())
    }

    /// 指定ビットを取得（0/1）
    pub fn get_bit(&self, i: usize) -> u8 {
        let byte_index = i / 8;
        let bit_index = 7 - (i % 8);
        if byte_index >= self.0.len() {
            0
        } else {
            (self.0[byte_index] >> bit_index) & 1
        }
    }

    /// 下位を検索するときに使用する範囲の終わりを示す
    pub fn generate_bottom_prefix_end(&self) -> BitVec {
        todo!()
    }

    /// 2ビット単位で prefix を生成するイテレータ
    /// 有効ビットが切れた時点で終了
    pub fn generate_top_prefix(&self) -> impl Iterator<Item = BitVec> + '_ {
        let input = &self.0;

        // 最後の有効ビットを探す
        let mut last_bit = 0;
        for (byte_index, &b) in input.iter().enumerate() {
            if b != 0 {
                for bit_index in 0..8 {
                    if (b & (1 << (7 - bit_index))) != 0 {
                        let pos = byte_index * 8 + bit_index;
                        if pos > last_bit {
                            last_bit = pos;
                        }
                    }
                }
            }
        }
        let total_bits = last_bit + 1;

        let mut bit_pos = 0;
        std::iter::from_fn(move || {
            if bit_pos + 1 >= total_bits {
                return None; // 有効ビットが残っていない場合は終了
            }

            // 2ビット取得して新しい BitVec を作成
            let mut chunk = vec![0u8; 1]; // 1バイト分確保
            for i in 0..2 {
                let pos = bit_pos + i;
                let byte_index = pos / 8;
                let bit_index = 7 - (pos % 8);
                let bit = (input[byte_index] >> bit_index) & 1;
                chunk[0] |= bit << (7 - i); // 左詰め
            }

            bit_pos += 2;
            Some(BitVec(chunk))
        })
    }

    /// self の先頭が prefix と一致するか判定
    pub fn starts_with(&self, prefix: &BitVec) -> bool {
        todo!()
    }
}

impl Deref for BitVec {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BitVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
