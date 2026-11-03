#![no_std]
#![forbid(unsafe_code)]

/// Experiment ABI v1. Integer fields only; reserved is always initialized.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RangeReply {
    pub status: u32,
    pub reserved: u32,
    pub start: u64,
    pub end: u64,
}

pub fn checked_range(offset: u64, length: u64, capacity: u64) -> RangeReply {
    let failure = |status| RangeReply {
        status,
        reserved: 0,
        start: 0,
        end: 0,
    };
    if offset > capacity {
        return failure(1);
    }
    if length > capacity - offset {
        return failure(2);
    }
    RangeReply {
        status: 0,
        reserved: 0,
        start: offset,
        end: offset + length,
    }
}
