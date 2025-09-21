use kasane_logic::id::{DimensionRange, coordinates::Point};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::io::ValueEntry;

//共通型

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AllOrChoose<T> {
    Choose(T),
    All,
}

// ---------------------- Space管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DropSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateKey {
    pub space_name: String,
    pub key_name: String,
    pub key_type: KeyType,
    pub key_mode: KeyMode,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum KeyMode {
    UniqueKey,
    MultiKey,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum KeyType {
    INT,
    BOOLEAN,
    TEXT,
    FLOAT,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DropKey {
    pub space_name: String,
    pub key_name: String,
}

// ---------------------- Value管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InsertValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

// #[derive(Debug, Serialize, Deserialize, Clone)]
// pub struct UpdateValue {
//     pub space_name: String,
//     pub key_name: String,
//     pub range: Range,
//     pub value: ValueEntry,
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SelectValue {
    pub space_name: String,
    pub key_names: Vec<String>,
    pub range: Range,
    pub vertex: bool,
    pub center: bool,
    pub id_string: bool,
    pub id_pure: bool,
}

// ---------------------- Range & Function ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Range {
    Function(Function),
    Prefix(Prefix),
    IdSet(Vec<IdInput>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdInput {
    pub z: u8,
    pub f: DimensionRange<i32>,
    pub x: DimensionRange<u32>,
    pub y: DimensionRange<u32>,
    pub i: u32,
    pub t: DimensionRange<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Spot {
    pub point1: Point,
    pub zoom: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Line {
    pub point1: Point,
    pub point2: Point,
    pub zoom: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Triangle {
    pub point1: Point,
    pub point2: Point,
    pub point3: Point,
    pub zoom: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterValue {
    pub space_name: String,
    pub key_name: String,
    pub filter: FilterType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FilterType {
    HasValue,
    FilterBOOLEAN(FilterBOOLEAN),
    FilterINT(FilterINT),
    FilterTEXT(FilterTEXT),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FilterBOOLEAN {
    IsTrue,
    IsFalse,
    Equals(bool),
    NotEquals(bool),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FilterFOLAT {
    Equal(f32),
    NotEqual(f32),
    GreaterThan(f32),
    GreaterEqual(f32),
    LessThan(f32),
    LessEqual(f32),
    Between(f32, f32),
    In(Vec<f32>),
    NotIn(Vec<f32>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FilterINT {
    Equal(i32),
    NotEqual(i32),
    GreaterThan(i32),
    GreaterEqual(i32),
    LessThan(i32),
    LessEqual(i32),
    Between(i32, i32),
    In(Vec<i32>),
    NotIn(Vec<i32>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FilterTEXT {
    Equal(String),
    NotEqual(String),
    Contains(String),
    NotContains(String),
    StartsWith(String),
    EndsWith(String),
    CaseInsensitiveEqual(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Function {
    Spot(Spot),
    Line(Line),
    Triangle(Triangle),
    //FilterValue(FilterValue),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Prefix {
    AND(Vec<Range>),
    OR(Vec<Range>),
    // XOR(Vec<Range>),
    // NOT(Vec<Range>),
}

// ---------------------- Key / Space情報 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShowKeys {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InfoKey {
    pub space_name: String,
    pub key_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InfoSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShowValues {
    pub space_name: String,
    pub key_name: String,
}

// ---------------------- User管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateUser {
    pub user_name: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DropUser {
    pub user_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InfoUser {
    pub user_name: String,
}

// ---------------------- 権限管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrantDatabase {
    pub user_name: String,
    pub command: AllOrChoose<Vec<CommandDatabase>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum CommandDatabase {
    CreateSpace,
    DropSpace,
    ShowSpaces,
    Version,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrantSpacePrivilege {
    pub user_name: String,
    pub target_space: AllOrChoose<Vec<String>>,
    pub command: AllOrChoose<Vec<CommandSpace>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum CommandSpace {
    CreateKey,
    DropKey,
    InfoSpace,
    ShowKeys,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrantKeyPrivilege {
    pub user_name: String,
    pub target_space: String,
    pub target_key: AllOrChoose<Vec<String>>,
    pub command: AllOrChoose<Vec<CommandKey>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CommandKey {
    InsertValue,
    PatchValue,
    UpdateValue,
    DropKey,
    SelectValue,
    InfoKey,
    ShowValues,
    FilterValue,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RevokeDatabase {
    pub user_name: String,
    pub command: AllOrChoose<Vec<CommandDatabase>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RevokeSpacePrivilege {
    pub user_name: String,
    pub target_space: AllOrChoose<Vec<String>>,
    pub command: AllOrChoose<Vec<CommandSpace>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RevokeKeyPrivilege {
    pub user_name: String,
    pub target_space: String,
    pub target_key: AllOrChoose<Vec<String>>,
    pub command: AllOrChoose<Vec<CommandKey>>,
}

// ---------------------- Packet & Command ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Command {
    //データベース操作系
    CreateSpace(CreateSpace),
    DropSpace(DropSpace),
    InfoSpace(InfoSpace),
    ShowSpaces,
    Version,

    //Key操作系
    CreateKey(CreateKey),
    DropKey(DropKey),
    ShowKeys(ShowKeys),
    InfoKey(InfoKey),

    //Value操作系
    InsertValue(InsertValue),
    PatchValue(PatchValue),
    //UpdateValue(UpdateValue),
    DeleteValue(DeleteValue),
    SelectValue(SelectValue),
    ShowValues(ShowValues),

    //ツール系
    //Transaction(Vec<Command>),

    //ユーザー操作系
    CreateUser(CreateUser),
    DropUser(DropUser),
    InfoUser(InfoUser),
    ShowUsers,
    // //権限付与系
    // GrantDatabase(GrantDatabase),
    // GrantSpacePrivilege(GrantSpacePrivilege),
    // GrantKeyPrivilege(GrantKeyPrivilege),

    // //権限取り上げ系
    // RevokeDatabase(RevokeDatabase),
    // RevokeSpacePrivilege(RevokeSpacePrivilege),
    // RevokeKeyPrivilege(RevokeKeyPrivilege),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Packet {
    pub session: String,
    pub command: Vec<Command>,
}

pub fn parser(value: &Value) -> Result<Packet, serde_json::Error> {
    serde_json::from_value(value.clone())
}
