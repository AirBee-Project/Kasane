use kasane_logic::{FlexId, RangeId, SingleId};

use crate::models::ValueLiteral;

#[derive(Debug, Clone)]
pub enum GetDataResponse {
    Single(GetDataResponseSingle),
    Range(GetDataResponseRange),
    Flex(GetDataResponseFlex),
}

#[derive(Debug, Clone)]
pub struct GetDataResponseSingle {
    pub dictionary: Vec<ValueLiteral>,
    pub data: Vec<DataGroup<SingleId>>,
}

#[derive(Debug, Clone)]
pub struct GetDataResponseRange {
    pub dictionary: Vec<ValueLiteral>,
    pub data: Vec<DataGroup<RangeId>>,
}

#[derive(Debug, Clone)]
pub struct GetDataResponseFlex {
    pub dictionary: Vec<ValueLiteral>,
    pub data: Vec<DataGroup<FlexId>>,
}

#[derive(Debug, Clone)]
pub struct DataGroup<T> {
    pub value_ref: usize,
    pub spatial_ids: Vec<T>,
}
