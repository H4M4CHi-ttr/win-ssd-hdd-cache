//! Shared inert data only. No freshness, lease or publication decisions here.
use cache_types::trace::{IoStatus, IoTarget};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataCell {
    pub(crate) version: u64,
    pub(crate) value: u8,
    pub(crate) known: bool,
    pub(crate) covered: bool,
}
impl DataCell {
    pub const fn value(self) -> u8 {
        self.value
    }
    pub const fn known(self) -> bool {
        self.known
    }
    pub(crate) const fn initial(value: u8) -> Self {
        Self {
            version: 0,
            value,
            known: true,
            covered: true,
        }
    }
}
const _: () = assert!(std::mem::size_of::<DataCell>() <= 16);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerResult {
    pub(crate) status: IoStatus,
    pub(crate) reported: u64,
    pub(crate) cells: Vec<DataCell>,
}
impl CallerResult {
    pub const fn status(&self) -> IoStatus {
        self.status
    }
    pub const fn reported(&self) -> u64 {
        self.reported
    }
    pub fn data(&self) -> Vec<u8> {
        self.cells.iter().map(|c| c.value).collect()
    }
    pub fn known(&self) -> Vec<bool> {
        self.cells.iter().map(|c| c.known).collect()
    }
    pub fn cells(&self) -> &[DataCell] {
        &self.cells
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub start: usize,
    pub len: usize,
}
impl Span {
    pub fn end(self) -> usize {
        self.start + self.len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IoDescriptor {
    pub id: u64,
    pub parent: u64,
    pub target: IoTarget,
    pub span: Span,
    pub slot: Option<u64>,
    pub source: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Completion {
    pub status: IoStatus,
    pub reported: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestKind {
    Read,
    Write,
    Discard,
    Flush,
    Fill,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RequestDescriptor<'a> {
    pub id: u64,
    pub span: Span,
    pub kind: RequestKind,
    pub data: &'a [u8],
    pub fua: bool,
    pub admitted_at: usize,
}

pub const MAX_LIVE: usize = 1024;
pub const MAX_OWNED_CELLS: usize = 16_777_216;

/// Logical-cell accounting, not RSS or Windows nonpaged-pool measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceUsage {
    pub owned_cells: usize,
    pub peak_owned_cells: usize,
    pub oracle_owned_cells: usize,
    pub peak_oracle_owned_cells: usize,
    pub fixed_media_cells: usize,
    pub oracle_fixed_media_cells: usize,
}
