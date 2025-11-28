use std::collections::{HashMap, HashSet};

use kasane_logic::encode_id_map::EncodeIDMap;

use crate::{
    configuration::Configuration,
    interface::input::{KeyType, ValueEntry},
};

pub mod key;
pub mod value;

#[derive(Debug, Clone)]
pub struct Storage {
    ///時空間IDを記録する
    inner: HashMap<String, (KeyType, EncodeIDMap<ValueEntry>)>,
}

impl Storage {
    pub fn new(conf: Configuration, import: Option<Vec<Storage>>) -> Storage {
        //import経由で外のストレージのデータをインポートする
        Storage {
            inner: HashMap::new(),
        }
    }

    pub fn export(&self) -> Storage {
        self.clone()
    }
}
