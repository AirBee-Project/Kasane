// Module declarations and re-exports
pub mod key;
pub mod space;
pub mod value;
pub mod version;

// Re-export types from submodules
pub use key::*;
pub use space::*;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
pub use value::*;
pub use version::*;

#[cfg(feature = "serde")]
use serde::Serialize;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "output/output.ts"))]
pub enum Output {
    Success,

    //データベース操作系
    InfoSpace(InfoSpace),
    ShowSpaces(ShowSpaces),
    Version(Version),

    //Key操作系
    Showkeys(Showkeys),
    InfoKey(InfoKey),

    //Value操作系
    SelectValue(Vec<Value>),
    ShowValues(Vec<Value>),
}
