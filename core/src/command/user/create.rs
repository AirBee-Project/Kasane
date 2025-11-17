use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    io::full::Storage,
    json::{input::CreateUser, output::Output},
    location,
    user_error::UserError,
};
use regex::Regex;

pub fn create_user(v: CreateUser, s: Arc<Storage>) -> Result<Output, UserError> {
    //ユーザー命名規則の検証
    match valid_name(&v.user_name) {
        Ok(_) => {}
        Err(e) => {
            return Err(UserError::UserNameVaildationError {
                name: v.user_name,
                reason: e,
                location: location!(),
            });
        }
    }

    //パスワードの検証
    let has_letter = Regex::new(r"[A-Za-z]").expect("Invalid regex pattern");
    let has_digit = Regex::new(r"\d").expect("Invalid regex pattern");

    if v.password.len() < 10
        || !has_letter.is_match(&v.password)
        || !has_digit.is_match(&v.password)
    {
        return Err(UserError::UserPasswordVaildationError {
            reason:
                "Password must be at least 10 characters long and contain both letters and numbers.",
            location: location!(),
        });
    }

    s.create_user(&v.user_name, &v.password)
}
