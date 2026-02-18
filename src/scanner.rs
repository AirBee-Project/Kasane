use crate::{
    Kasane,
    error::Error,
    tables::{FiledRank, FlexIdRank, SerializableRoaringTreemap},
};
use kasane_logic::{Block, FlexId, RoaringTreemap, Segment, fast_intersect};
use redb::{ReadableTable, Table, WriteTransaction};
use std::cell::OnceCell;

pub trait Scanner<'txn>: Sized {
    fn f(
        &'txn self,
    ) -> Result<
        redb::Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
        redb::TableError,
    >;
    fn x(
        &'txn self,
    ) -> Result<
        redb::Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
        redb::TableError,
    >;
    fn y(
        &'txn self,
    ) -> Result<
        redb::Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
        redb::TableError,
    >;

    fn flex_id_scan_plan<'a, T: Block>(
        &'txn self, // ここも修正
        filed_rank: FiledRank,
        target: T,
    ) -> Result<FlexIdScanPlan<'txn>, Error> {
        FlexIdScanPlan::new(filed_rank, self, target)
    }
}

// --- FlexIdScanPlan ---

#[derive(Debug)]
pub struct FlexIdScanPlan<'txn> {
    f: Vec<SegmentFamily<'txn>>,
    x: Vec<SegmentFamily<'txn>>,
    y: Vec<SegmentFamily<'txn>>,
}
impl<'txn> FlexIdScanPlan<'txn> {
    pub fn new<T: Block, S: Scanner<'txn>>(
        filed_rank: FiledRank,
        scanner: &'txn S,
        target: T,
    ) -> Result<Self, Error> {
        let segmentation = target.segmentation();
        let f = segmentation
            .f
            .into_iter()
            .map(|s| {
                Ok(SegmentFamily::new(
                    filed_rank,
                    s,
                    scanner.f().map_err(Error::from)?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let x = segmentation
            .x
            .into_iter()
            .map(|s| {
                Ok(SegmentFamily::new(
                    filed_rank,
                    s,
                    scanner.x().map_err(Error::from)?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let y = segmentation
            .y
            .into_iter()
            .map(|s| {
                Ok(SegmentFamily::new(
                    filed_rank,
                    s,
                    scanner.y().map_err(Error::from)?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(Self { f, x, y })
    }

    pub fn scan<'b>(&'b self) -> impl Iterator<Item = FlexIdScanner<'b, 'txn>> {
        self.f.iter().flat_map(move |f| {
            self.x.iter().flat_map(move |x| {
                self.y.iter().map(move |y| FlexIdScanner {
                    f,
                    x,
                    y,
                    parent: OnceCell::new(),
                    children: OnceCell::new(),
                })
            })
        })
    }
}

pub struct FlexIdScanner<'b, 'txn> {
    f: &'b SegmentFamily<'txn>,
    x: &'b SegmentFamily<'txn>,
    y: &'b SegmentFamily<'txn>,

    parent: OnceCell<Option<FlexIdRank>>,
    children: OnceCell<RoaringTreemap>,
}

impl<'b, 'txn> FlexIdScanner<'b, 'txn> {
    pub fn flex_id(&self) -> FlexId {
        FlexId::new(self.f().clone(), self.x().clone(), self.y().clone())
    }

    pub fn parent(&self) -> Result<Option<FlexIdRank>, Error> {
        if let Some(res) = self.parent.get() {
            return Ok(*res);
        }

        let f = self.f.parents()?;
        let x = self.x.parents()?;
        let y = self.y.parents()?;

        let intersection = f & x & y;

        #[cfg(debug_assertions)]
        if intersection.len() > 1 {
            panic!("Critical: 複数の親が検知されました");
        }

        let res = intersection.iter().next().map(|id| id as FlexIdRank);
        let _ = self.parent.set(res);
        Ok(res)
    }

    pub fn children(&self) -> Result<&RoaringTreemap, Error> {
        if let Some(res) = self.children.get() {
            return Ok(res);
        }

        let f = self.f.children()?;
        let x = self.x.children()?;
        let y = self.y.children()?;

        let intersection = fast_intersect([f, x, y]);
        let _ = self.children.set(intersection);
        Ok(self.children.get().expect("Just initialized"))
    }

    pub fn partial_overlaps(&self) -> Result<RoaringTreemap, Error> {
        let mut all = self.all()?;
        all -= self.children()?;
        if let Some(parent_rank) = self.parent()? {
            all.remove(parent_rank as u64);
        }
        Ok(all)
    }

    pub fn all(&self) -> Result<RoaringTreemap, Error> {
        let f = self.f.parents()? | self.f.children()?;
        let x = self.x.parents()? | self.x.children()?;
        let y = self.y.parents()? | self.y.children()?;
        Ok(fast_intersect([&f, &x, &y]))
    }

    pub fn f(&self) -> &Segment {
        &self.f.segment
    }
    pub fn x(&self) -> &Segment {
        &self.x.segment
    }
    pub fn y(&self) -> &Segment {
        &self.y.segment
    }
}

// --- SegmentFamily ---

#[derive(Debug)]
pub struct SegmentFamily<'txn> {
    filed_rank: FiledRank,
    segment: Segment,
    parents: OnceCell<RoaringTreemap>,
    children: OnceCell<RoaringTreemap>,
    btree: Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
}

impl<'txn> SegmentFamily<'txn> {
    fn new(
        filed_rank: FiledRank,
        segment: Segment,
        btree: Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
    ) -> Self {
        Self {
            filed_rank,
            segment,
            parents: OnceCell::new(),
            children: OnceCell::new(),
            btree,
        }
    }

    fn parents(&self) -> Result<&RoaringTreemap, Error> {
        if let Some(res) = self.parents.get() {
            return Ok(res);
        }

        let mut result = RoaringTreemap::new();
        for parent_segment in self.segment.self_and_parents() {
            let key = (self.filed_rank, parent_segment.into());
            if let Some(access) = self.btree.get(key)? {
                result |= access.value().as_treemap();
            }
        }

        let _ = self.parents.set(result);
        Ok(self.parents.get().unwrap())
    }

    fn children(&self) -> Result<&RoaringTreemap, Error> {
        if let Some(res) = self.children.get() {
            return Ok(res);
        }

        let mut result = RoaringTreemap::new();
        let start_key = (self.filed_rank, self.segment.clone().into());

        let range_scan = if let Some(end_seg) = self.segment.descendant_range_end() {
            let end_key = (self.filed_rank, end_seg.into());
            self.btree.range(start_key..end_key)?
        } else {
            let guard_key = (self.filed_rank + 1, [0u8; Segment::ARRAY_LENGTH]);
            self.btree.range(start_key..guard_key)?
        };

        for item in range_scan {
            let (_key, value_access) = item?;
            result |= value_access.value().as_treemap();
        }

        let _ = self.children.set(result);
        Ok(self.children.get().expect("Just initialized"))
    }
}
