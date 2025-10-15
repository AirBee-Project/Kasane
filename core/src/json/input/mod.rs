use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    io::full::tools::value_entry::ValueEntry,
    r#type::{point::Point, spacetimeid::DimensionRange},
};

// ---------------------- Space管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DropSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateKey {
    pub space_name: String,
    pub key_name: String,
    pub key_type: KeyType,
    pub key_mode: KeyMode,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum KeyMode {
    UniqueKey,
    MultiKey,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum KeyType {
    BOOLEAN,
    TEXT,
    FLOAT,
    INT,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DropKey {
    pub space_name: String,
    pub key_name: String,
}

// ---------------------- Value管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InsertValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PatchValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
    pub value: ValueEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteValue {
    pub space_name: String,
    pub key_name: String,
    pub range: Range,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub enum Range {
    Function(Function),
    Prefix(Prefix),
    IdSet(Vec<IdInput>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IdInput {
    pub z: u8,
    pub f: [Option<u64>; 2],
    pub x: [Option<u64>; 2],
    pub y: [Option<u64>; 2],
    pub i: u32,
    pub t: [Option<u64>; 2],
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Spot {
    pub point1: Point,
    pub zoom: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub point1: Point,
    pub point2: Point,
    pub zoom: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Triangle {
    pub point1: Point,
    pub point2: Point,
    pub point3: Point,
    pub zoom: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FilterValue {
    pub space_name: String,
    pub key_name: String,
    pub filter: FilterType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FilterType {
    HasValue,
    FilterBOOLEAN(FilterBOOLEAN),
    FilterINT(FilterINT),
    FilterTEXT(FilterTEXT),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FilterBOOLEAN {
    IsTrue,
    IsFalse,
    Equals(bool),
    NotEquals(bool),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub enum Function {
    Spot(Spot),
    Line(Line),
    Triangle(Triangle),
    //FilterValue(FilterValue),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Prefix {
    AND(Vec<Range>),
    OR(Vec<Range>),
    // XOR(Vec<Range>),
    NOT(Vec<Range>),
}

// ---------------------- Key / Space情報 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShowKeys {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InfoKey {
    pub space_name: String,
    pub key_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InfoSpace {
    pub space_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShowValues {
    pub space_name: String,
    pub key_name: String,
}

// ---------------------- User管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateUser {
    pub user_name: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DropUser {
    pub user_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InfoUser {
    pub user_name: String,
}

// ---------------------- 権限管理 ----------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantDatabase {
    pub user_name: String,
    pub command: Vec<DatabaseCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum DatabaseCommand {
    ALL = 0,
    CreateSpace = 1,
    DropSpace = 2,
    ShowSpaces = 3,
    Version = 4,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantSpace {
    pub user_name: String,
    pub target_space: Vec<String>,
    pub command: Vec<SpaceCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SpaceCommand {
    ALL,
    CreateKey,
    DropKey,
    InfoSpace,
    ShowKeys,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantKey {
    pub user_name: String,
    pub target_space: String,
    pub target_key: Vec<String>,
    pub command: Vec<KeyCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum KeyCommand {
    ALL,
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
#[serde(rename_all = "camelCase")]
pub struct RevokeDatabase {
    pub user_name: String,
    pub command: Vec<DatabaseCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RevokeSpace {
    pub user_name: String,
    pub target_space: Vec<String>,
    pub command: Vec<SpaceCommand>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RevokeKey {
    pub user_name: String,
    pub target_space: String,
    pub target_key: Vec<String>,
    pub command: Vec<KeyCommand>,
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
    UpdateValue(UpdateValue),
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

    //権限付与系
    GrantDatabase(GrantDatabase),
    GrantSpace(GrantSpace),
    GrantKeyPrivilege(GrantKey),

    //権限取り上げ系
    RevokeDatabase(RevokeDatabase),
    RevokeSpacePrivilege(RevokeSpace),
    RevokeKeyPrivilege(RevokeKey),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Packet {
    pub session: String,
    pub command: Vec<Command>,
}

pub fn parser(value: &Value) -> Result<Packet, serde_json::Error> {
    serde_json::from_value(value.clone())
}
