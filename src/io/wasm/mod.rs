use std::{
    clone,
    collections::{HashMap, HashSet},
};

use kasane_logic::encode_id_map::EncodeIDMap;

use crate::{
    configuration::Configuration,
    interface::input::{KeyType, ValueEntry},
};

#[derive(Debug, Clone)]
pub struct Storage {
    ///時空間IDを記録する
    inner: HashMap<String, EncodeIDMap<ValueEntry>>,

    ///Keyごとに型を記録する
    r#type: HashMap<String, String>,
}

impl Storage {
    pub fn new(conf: Configuration, import: Option<Vec<Storage>>) -> Storage {
        //もしImportするものがあればそれをImportに含めると、初期状態で追加される
        Storage {
            inner: HashMap::new(),
            r#type: HashMap::new(),
        }
    }

    pub fn export(&self) -> Storage {
        self.clone()
    }
}
