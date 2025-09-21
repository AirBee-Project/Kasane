use crate::{error::Error, io::tools::keytype_id::keytype_id, json::input::KeyType};

pub fn key_bytes(space_id: &[u8], keyname: &str, keytype: KeyType) -> Result<String, Error> {
    let space_id_str = std::str::from_utf8(space_id).map_err(|_| Error::NnKnown)?;
    Ok(format!(
        "{}:{}:{}",
        space_id_str,
        keyname,
        keytype_id(keytype)
    ))
}
