use crate::io::io::Storage;

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::ShowKeys, output::Output},
    location,
    user_error::UserError,
};
use std::sync::Arc;
pub fn show_keys(v: ShowKeys, s: Arc<Storage>) -> Result<Output, UserError> {
    //ストレージに対して操作を実行する
    s.show_keys(&v.space_name)
}
