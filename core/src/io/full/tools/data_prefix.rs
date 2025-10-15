#[repr(u8)]
pub enum Data {
    Space = 0,
    Key = 1,
    User = 2,
    Password = 3,
    DatabaseGrant = 4,
    SpaceGrant = 5,
    SpaceGrantTarget = 6,
    KeyGrant = 6,
    UserGrant = 7,
    Interval = 8,
    Value = 9,
}
