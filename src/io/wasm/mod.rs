use std::collections::{HashMap, HashSet};

use kasane_logic::encode_id_map::EncodeIDMap;

use crate::{configuration::Configuration, interface::input::ValueEntry};

pub mod space;

#[derive(Debug)]
pub struct Storage {
    inner: HashMap<String, HashMap<String, EncodeIDMap<ValueEntry>>>,
}

impl Storage {
    pub fn new(conf: Configuration) -> Storage {
        Storage {
            inner: HashMap::new(),
        }
    }
}
