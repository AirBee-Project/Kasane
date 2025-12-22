use kasane_logic::geometry::coordinate::Coordinate;

pub enum KeyType {
    Text,
    Float,
    Int,
    Boolean,
}

pub enum Range {
    Function(Function),
    Calculation(Calculation),
    Ids(Vec<SpaceTimeIDInput>),
    FilterValue(FilterValue),
}

pub struct SpaceTimeIDInput {
    pub z: u8,
    pub f: [Option<i64>; 2],
    pub x: [Option<u64>; 2],
    pub y: [Option<u64>; 2],
}

pub struct Point {
    pub z: u8,
    pub point1: Coordinate,
}

pub struct Line {
    pub z: u8,
    pub point1: Coordinate,
    pub point2: Coordinate,
}

pub struct Triangle {
    pub z: u8,
    pub point1: Coordinate,
    pub point2: Coordinate,
    pub point3: Coordinate,
}

pub struct FilterValue {
    pub key_name: String,
    pub filter: Filter,
}

pub enum Filter {
    FilterBoolean(FilterBoolean),
    FilterInt(FilterInt),
    FilterFloat(FilterFloat),
    FilterText(FilterText),
}

pub enum FilterBoolean {
    HasValue,
    IsTrue,
    IsFalse,
    Equals(bool),
    NotEquals(bool),
}

pub enum FilterFloat {
    HasValue,
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

pub enum FilterInt {
    HasValue,
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

pub enum FilterText {
    HasValue,
    Equal(String),
    NotEqual(String),
    Contains(String),
    NotContains(String),
    StartsWith(String),
    EndsWith(String),
    CaseInsensitiveEqual(String),
}

pub enum Function {
    Point(Point),
    Line(Line),
    Triangle(Triangle),
}

pub enum Calculation {
    AND(Vec<Range>),
    OR(Vec<Range>),
    DIFF {
        base: Box<Range>,   // 引かれる集合（元集合）
        remove: Box<Range>, // 引く集合
    },
}
