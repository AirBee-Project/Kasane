use crate::json::input::KeyType;

pub fn keytype_id(keytype: KeyType) -> u8 {
    match keytype {
        KeyType::INT => 1,
        KeyType::BOOLEAN => 2,
        KeyType::TEXT => 3,
        KeyType::FLOAT => 4,
    }
}

pub fn id_keytype(id: u8) -> KeyType {
    match id {
        1 => KeyType::INT,
        2 => KeyType::BOOLEAN,
        3 => KeyType::TEXT,
        4 => KeyType::FLOAT,
        _ => panic!("ありえん"), // デフォルト
    }
}
