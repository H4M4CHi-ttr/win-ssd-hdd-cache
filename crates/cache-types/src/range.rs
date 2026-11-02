//! Checked byte ranges for the portable host model, not device alignment rules.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineSpan {
    pub first: u64,
    pub count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeError {
    OffsetPastEnd,
    LengthPastEnd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineSize {
    K4,
    K8,
    K16,
    K32,
    K64,
}

pub fn checked_range(offset: u64, length: u64, capacity: u64) -> Result<ByteRange, RangeError> {
    if offset > capacity {
        return Err(RangeError::OffsetPastEnd);
    }
    if length > capacity - offset {
        return Err(RangeError::LengthPastEnd);
    }
    Ok(ByteRange {
        start: offset,
        end: offset + length,
    })
}

pub fn line_span(
    offset: u64,
    length: u64,
    capacity: u64,
    size: LineSize,
) -> Result<LineSpan, RangeError> {
    let range = checked_range(offset, length, capacity)?;
    let line_bytes = match size {
        LineSize::K4 => 4096,
        LineSize::K8 => 8192,
        LineSize::K16 => 16384,
        LineSize::K32 => 32768,
        LineSize::K64 => 65536,
    };
    let first = range.start / line_bytes;
    let count = if length == 0 {
        0
    } else {
        (range.end - 1) / line_bytes - first + 1
    };
    Ok(LineSpan { first, count })
}
