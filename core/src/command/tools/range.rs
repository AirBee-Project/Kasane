use crate::{
    json::input::Range,
    r#type::{space_time_id::SpaceTimeId, space_time_id_set::SpaceTimeIdSet},
    user_error::UserError,
};

enum DatabaseRangePrefix {
    AND(Vec<DatabaseRange>),
    OR(Vec<DatabaseRange>),
    NOT(Vec<DatabaseRange>),
}

enum DatabaseRange {
    SpaceTimeIdSet(SpaceTimeIdSet),
    ReadDatabase(),
    DatabaseRangePrefix(DatabaseRangePrefix),
}

pub fn range(range: Range) -> Result<DatabaseRange, UserError> {
    todo!()
}

pub fn database_range(database_range: DatabaseRange) -> Result<DatabaseRange, UserError> {
    match database_range {
        DatabaseRange::SpaceTimeIdSet(space_time_id_set) => todo!(),
        DatabaseRange::ReadDatabase() => todo!(),
        DatabaseRange::DatabaseRangePrefix(prefix) => match prefix {
            DatabaseRangePrefix::AND(and) => for ele in and {},
            DatabaseRangePrefix::OR(or) => todo!(),
            DatabaseRangePrefix::NOT(not) => todo!(),
        },
    }
}
