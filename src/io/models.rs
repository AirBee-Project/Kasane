use std::collections::{BTreeSet, HashSet};

use kasane_logic::bit_vec::BitVec;
use redb::Key;
//ここではKV-Storeに入れる共通の型を定義する
use redb::Value;

//Defは内部用の型である

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentInfoDef {
    pub pointers: BTreeSet<u64>,
    pub under_count: u64,
}

impl Value for SegmentInfoDef {
    type SelfType<'a>
        = SegmentInfoDef
    where
        Self: 'a;

    type AsBytes<'a>
        = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut offset = 0;

        // count
        let under_count = u64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .expect("invalid SegmentInfoDef"),
        );
        offset += 8;

        // pointers length
        let len = u64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .expect("invalid SegmentInfoDef"),
        ) as usize;
        offset += 8;

        let mut pointers = BTreeSet::new();

        for _ in 0..len {
            let ptr = u64::from_le_bytes(
                data[offset..offset + 8]
                    .try_into()
                    .expect("invalid SegmentInfoDef"),
            );
            offset += 8;
            pointers.insert(ptr);
        }

        SegmentInfoDef {
            pointers,
            under_count,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = Vec::new();

        // count
        bytes.extend_from_slice(&value.under_count.to_le_bytes());

        // pointers length
        let len = value.pointers.len() as u64;
        bytes.extend_from_slice(&len.to_le_bytes());

        // BTreeSet は常に昇順
        for ptr in &value.pointers {
            bytes.extend_from_slice(&ptr.to_le_bytes());
        }

        bytes
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("SegmentInfoDef")
    }
}

#[derive(Debug)]
pub struct SegmentDef {
    field_id: u64,
    bit_vec: BitVec,
}

impl Value for SegmentDef {
    type SelfType<'a>
        = SegmentDef
    where
        Self: 'a;

    type AsBytes<'a>
        = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        use std::convert::TryInto;

        let field_id = u64::from_le_bytes(data[0..8].try_into().expect("invalid SegmentDef"));

        let bit_bytes = &data[8..];
        let bit_vec = BitVec::from_slice(bit_bytes);

        SegmentDef { field_id, bit_vec }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&value.field_id.to_le_bytes());

        let bit_vec = value.bit_vec.clone();

        bytes.extend_from_slice(&bit_vec.as_slice());

        bytes
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("SegmentDef")
    }
}

impl Key for SegmentDef {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // field_id
        let id1 = u64::from_le_bytes(data1[0..8].try_into().unwrap());
        let id2 = u64::from_le_bytes(data2[0..8].try_into().unwrap());

        match id1.cmp(&id2) {
            Ordering::Equal => {}
            other => return other,
        }

        // bit bytes
        let bits1 = &data1[8..];
        let bits2 = &data2[8..];

        bits1.cmp(bits2)
    }
}

#[derive(Debug)]
pub struct EncodeIdDef {
    f: BitVec,
    x: BitVec,
    y: BitVec,
    value: Vec<u8>, // 可変長に変更
}

impl Value for EncodeIdDef {
    type SelfType<'a> = EncodeIdDef;
    type AsBytes<'a> = Vec<u8>;

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        todo!()
    }

    fn fixed_width() -> Option<usize> {
        None // 可変長のため
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        todo!()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("EncodeIdDef")
    }
}
