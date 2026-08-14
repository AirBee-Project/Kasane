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
        bytes.extend_from_slice(&item.0.into_bytes());
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
        let mut id = [0u8; 16];
        id.copy_from_slice(&bytes[0..16]);
        let id = crate::models::id::DatabaseId(uuid::Uuid::from_bytes(id));
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
        bytes.extend_from_slice(&item.0.into_bytes());
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
        let mut table_id = [0u8; 16];
        table_id.copy_from_slice(&bytes[0..16]);
        let table_id = crate::models::id::TableId(uuid::Uuid::from_bytes(table_id));

        let mut flex_id_bytes = [0u8; FlexId::ENCODED_LEN];
        flex_id_bytes.copy_from_slice(&bytes[16..16 + FlexId::ENCODED_LEN]);
        let flex_id = FlexId::decode(&flex_id_bytes)?;

        Ok((table_id, flex_id))
    }
}
