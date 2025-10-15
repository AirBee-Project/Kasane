#[repr(u8)]
pub enum Data {
    Space = 0,
    Key = 1,
    User = 2,
    Password = 3,
    Grant = 4,
    Interval = 5,
    Value = 6,
}
