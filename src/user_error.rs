#[cfg(feature = "file")]
use bcrypt::BcryptError;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum UserError {
    // ==================== Validation Errors ====================
    // バリデーションエラー: 名前やパスワードの検証で使用
    #[error("Invalid key name '{name}': {reason} (at {location})")]
    KeyNameValidationError {
        name: String,
        reason: &'static str,
        location: String,
    },

    #[error("Parse error: {message} (at {location})")]
    ParseError { message: String, location: String },

    // ==================== Entity Not Found Errors ====================
    // エンティティが存在しないエラー
    #[error("Key '{key_name}' not found (at {location})")]
    KeyNotFound { key_name: String, location: String },

    // ==================== Entity Already Exists Errors ====================
    // エンティティが既に存在するエラー
    #[error("Key '{key_name}' already exists (at {location})")]
    KeyAlreadyExists { key_name: String, location: String },
}

//Kasane-Logicのエラー型を変換
impl From<kasane_logic::error::Error> for UserError {
    fn from(err: kasane_logic::error::Error) -> Self {
        let location = format!("{}:{}", file!(), line!());

        match err {
            kasane_logic::error::Error::ZoomLevelOutOfRange { zoom_level } => {
                UserError::ParseError {
                    message: format!("Zoom level out of range: {}", zoom_level),
                    location,
                }
            }
            kasane_logic::error::Error::FOutOfRange { f, z } => UserError::ParseError {
                message: format!("F coordinate {} out of range for zoom level {}", f, z),
                location,
            },
            kasane_logic::error::Error::XOutOfRange { x, z } => UserError::ParseError {
                message: format!("X coordinate {} out of range for zoom level {}", x, z),
                location,
            },
            kasane_logic::error::Error::YOutOfRange { y, z } => UserError::ParseError {
                message: format!("Y coordinate {} out of range for zoom level {}", y, z),
                location,
            },
            kasane_logic::error::Error::LatitudeOutOfRange { latitude } => UserError::ParseError {
                message: format!("Latitude {} out of range (-90.0..=90.0)", latitude),
                location,
            },
            kasane_logic::error::Error::LongitudeOutOfRange { longitude } => {
                UserError::ParseError {
                    message: format!("Longitude {} out of range (-180.0..=180.0)", longitude),
                    location,
                }
            }
            kasane_logic::error::Error::AltitudeOutOfRange { altitude } => UserError::ParseError {
                message: format!(
                    "Altitude {} out of range (-33,554,432.0..=33,554,432.0)",
                    altitude
                ),
                location,
            },
            kasane_logic::error::Error::TimeOverflow { t, i } => UserError::ParseError {
                message: format!("Time overflow occurred: t={}, i={}", t, i),
                location,
            },
        }
    }
}
