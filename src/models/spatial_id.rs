use kasane_logic::{FlexId, RangeId, SingleId};

/// 単一、区間、または拡張（Flex）形式の空間IDを表す enum。
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum SpatialId {
    SingleId(SingleId),
    RangeId(RangeId),
    FlexId(FlexId),
}

impl From<SingleId> for SpatialId {
    fn from(id: SingleId) -> Self {
        Self::SingleId(id)
    }
}

impl From<RangeId> for SpatialId {
    fn from(id: RangeId) -> Self {
        Self::RangeId(id)
    }
}

impl From<FlexId> for SpatialId {
    fn from(id: FlexId) -> Self {
        Self::FlexId(id)
    }
}
