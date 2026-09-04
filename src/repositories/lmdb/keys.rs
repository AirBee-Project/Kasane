//! 複合キーの [`BytesEncode`](heed::BytesEncode) / [`BytesDecode`](heed::BytesDecode) 実装。
//! バイトの並べ方は TiKV 実装と揃えてあり、どちらでも同じ順序でキーが並ぶ。

use std::borrow::Cow;

use kasane_logic::FlexId;

pub struct DbIdAndName;

impl<'a> heed::BytesEncode<'a> for DbIdAndName {
    type EItem = (crate::models::id::DatabaseId, &'a str);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(16 + item.1.len());
        bytes.extend_from_slice(item.0.as_bytes());
        bytes.extend_from_slice(item.1.as_bytes());
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for DbIdAndName {
    type DItem = (crate::models::id::DatabaseId, &'a str);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() < 16 {
            return Err("invalid length".into());
        }
        let id = crate::models::id::DatabaseId::try_from(&bytes[..16])?;
        let name = std::str::from_utf8(&bytes[16..])?;
        Ok((id, name))
    }
}

pub struct TableIdAndFlexId;

impl<'a> heed::BytesEncode<'a> for TableIdAndFlexId {
    type EItem = (crate::models::id::TableId, FlexId);

    fn bytes_encode(
        item: &'a Self::EItem,
    ) -> Result<Cow<'a, [u8]>, Box<dyn std::error::Error + Send + Sync>> {
        let mut bytes = Vec::with_capacity(16 + FlexId::ENCODED_LEN);
        bytes.extend_from_slice(item.0.as_bytes());
        bytes.extend_from_slice(&item.1.encode());
        Ok(Cow::Owned(bytes))
    }
}

impl<'a> heed::BytesDecode<'a> for TableIdAndFlexId {
    type DItem = (crate::models::id::TableId, FlexId);

    fn bytes_decode(
        bytes: &'a [u8],
    ) -> Result<Self::DItem, Box<dyn std::error::Error + Send + Sync>> {
        if bytes.len() != 16 + FlexId::ENCODED_LEN {
            return Err("invalid length for TableIdAndFlexId".into());
        }
        let table_id = crate::models::id::TableId::try_from(&bytes[..16])?;
        let flex_id = FlexId::decode(bytes[16..].try_into()?)?;

        Ok((table_id, flex_id))
    }
}
