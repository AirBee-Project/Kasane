use std::cmp::Ordering;
use std::collections::BTreeSet;

use kasane_logic::bit_vec::EncodeSegment;
use redb::Key;
use redb::TypeName;
//ここではKV-Storeに入れる共通の型を定義する
use redb::Value;

//Defは内部用の型である

pub type EncodeIDPointer = u64;
pub type FieldID = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentInfoDef {
    pub pointers: BTreeSet<EncodeIDPointer>,
    pub descendant_count: u64,
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
        let descendant_count = u64::from_le_bytes(
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
            descendant_count,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = Vec::new();

        // count
        bytes.extend_from_slice(&value.descendant_count.to_le_bytes());

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
    field_id: FieldID,
    encode_segment: EncodeSegment,
}

impl Value for SegmentDef {
    type SelfType<'a>
        = SegmentDef
    where
        Self: 'a;

    type AsBytes<'a>
        = [u8; 18]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(8 + EncodeSegment::BYTE_LEN)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let (field_id_bytes, segment_bytes) = data.split_at(8);

        let field_id = u64::from_le_bytes(field_id_bytes.try_into().unwrap());

        let segment_array: &[u8; 10] = segment_bytes.try_into().unwrap();

        let encode_segment = EncodeSegment::from_bytes(segment_array);

        SegmentDef {
            field_id,
            encode_segment,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = [0u8; 18];

        bytes[..8].copy_from_slice(&value.field_id.to_le_bytes());
        bytes[8..].copy_from_slice(value.encode_segment.as_bytes());

        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("SegmentDef")
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
pub struct EncodeIDDef {
    f: EncodeSegment,
    x: EncodeSegment,
    y: EncodeSegment,

    //Valueは何が入るか不明なので可変長配列
    value: Vec<u8>,
}

impl Value for EncodeIDDef {
    type SelfType<'a> = EncodeIDDef;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut offset = 0;
        let n = EncodeSegment::BYTE_LEN;

        // f
        let f_bytes: &[u8; EncodeSegment::BYTE_LEN] = data[offset..offset + n].try_into().unwrap();
        let f = EncodeSegment::from_bytes(f_bytes);
        offset += n;

        // x
        let x_bytes: &[u8; EncodeSegment::BYTE_LEN] = data[offset..offset + n].try_into().unwrap();
        let x = EncodeSegment::from_bytes(x_bytes);
        offset += n;

        // y
        let y_bytes: &[u8; EncodeSegment::BYTE_LEN] = data[offset..offset + n].try_into().unwrap();
        let y = EncodeSegment::from_bytes(y_bytes);
        offset += n;

        // value length
        let len_bytes: [u8; 4] = data[offset..offset + 4].try_into().unwrap();
        let value_len = u32::from_le_bytes(len_bytes) as usize;
        offset += 4;

        // value
        let value = data[offset..offset + value_len].to_vec();

        EncodeIDDef { f, x, y, value }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        let n = EncodeSegment::BYTE_LEN;

        let mut bytes = Vec::with_capacity(n * 3 + 4 + value.value.len());

        bytes.extend_from_slice(value.f.as_bytes());
        bytes.extend_from_slice(value.x.as_bytes());
        bytes.extend_from_slice(value.y.as_bytes());

        let len = value.value.len() as u32;
        bytes.extend_from_slice(&len.to_le_bytes());

        bytes.extend_from_slice(&value.value);

        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("EncodeIdDef")
    }
}

#[derive(Debug)]
pub struct ValueDef {
    field_id: FieldID,
    value: Vec<u8>,
}

impl Value for ValueDef {
    type SelfType<'a>
        = ValueDef
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

        // field_id (BE)
        let field_id = u64::from_be_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        // value length
        let value_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        // value
        let value = data[offset..offset + value_len].to_vec();

        ValueDef { field_id, value }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = Vec::with_capacity(8 + 4 + value.value.len());

        // field_id (BE)
        bytes.extend_from_slice(&value.field_id.to_be_bytes());

        // value length
        let len = value.value.len() as u32;
        bytes.extend_from_slice(&len.to_le_bytes());

        // value
        bytes.extend_from_slice(&value.value);

        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("ValueDef")
    }
}
impl Key for ValueDef {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        // field_id は先頭 8 bytes（BE）
        let a = &data1[..8];
        let b = &data2[..8];

        a.cmp(b)
    }
}

#[derive(Debug)]
pub struct ValueInfoDef(BTreeSet<EncodeIDPointer>);

impl Value for ValueInfoDef {
    type SelfType<'a>
        = ValueInfoDef
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

        // 要素数
        let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        let mut set = BTreeSet::new();

        for _ in 0..count {
            let id = u64::from_be_bytes(data[offset..offset + 8].try_into().unwrap());
            set.insert(id);
            offset += 8;
        }

        ValueInfoDef(set)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        let mut bytes = Vec::with_capacity(4 + value.0.len() * 8);

        // 要素数
        let count = value.0.len() as u32;
        bytes.extend_from_slice(&count.to_le_bytes());

        // BTreeSet は昇順保証
        for id in &value.0 {
            bytes.extend_from_slice(&id.to_be_bytes());
        }

        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("ValueInfoDef")
    }
}
