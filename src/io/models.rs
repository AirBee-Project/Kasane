//ここではKV-Storeに入れる共通の型を定義する
use redb::Value;

#[derive(Debug)]
pub struct FieldInfo {
    //フィールド型を表す番号
    // アプリケーション側に変換の責任を持たすことで、redb側でのエラーを強制的にpanicしなくていいようにする
    //賛否のある実装ですね。型Firstではない。
    pub type_u8: u8,

    //フィールドに対して一意な番号を作る
    pub id: u64,
}

impl Value for FieldInfo {
    type SelfType<'a>
        = FieldInfo
    where
        Self: 'a;
    type AsBytes<'a>
        = [u8; 9]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(9)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut id_bytes = [0u8; 8];
        id_bytes.copy_from_slice(&data[1..9]);

        FieldInfo {
            type_u8: data[0],
            id: u64::from_le_bytes(id_bytes),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = [0u8; 9];
        bytes[0] = value.type_u8 as u8;
        bytes[1..9].copy_from_slice(&value.id.to_le_bytes());
        bytes
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("FieldInfo")
    }
}
