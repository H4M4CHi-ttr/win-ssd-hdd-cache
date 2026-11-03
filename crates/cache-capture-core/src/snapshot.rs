//! Copied integer metadata. These structs are not a transport or pointer ABI.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SubmitSnapshot {
    pub version: u32,
    pub major: u32,
    pub minor: u32,
    pub flags: u32,
    pub control_known: u32,
    pub range_known: u32,
    pub control_code: u32,
    pub reserved: u32,
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct CompletionSnapshot {
    pub version: u32,
    pub status: u32,
    pub information_known: u32,
    pub reserved: u32,
    pub information: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    InvalidSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Read,
    Write,
    Flush,
    Other,
}

/// A candidate at the observed stack, not validated physical-device mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeCandidate {
    offset: u64,
    length: u64,
}

impl RangeCandidate {
    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn length(self) -> u64 {
        self.length
    }
}

/// Validated scalar shape; no constructor bypasses `decode_submit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Submission {
    major: u8,
    minor: u8,
    flags: u8,
    operation: Operation,
    range: Option<RangeCandidate>,
    control_code: Option<u32>,
}

impl Submission {
    pub const fn major(self) -> u8 {
        self.major
    }

    pub const fn minor(self) -> u8 {
        self.minor
    }

    pub const fn flags(self) -> u8 {
        self.flags
    }

    pub const fn operation(self) -> Operation {
        self.operation
    }

    pub const fn range(self) -> Option<RangeCandidate> {
        self.range
    }

    pub const fn control_code(self) -> Option<u32> {
        self.control_code
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub status: u32,
    pub information: Option<u64>,
}

pub fn decode_submit(raw: SubmitSnapshot) -> Result<Submission, SnapshotError> {
    use SnapshotError::InvalidSnapshot;
    if raw.version != 1
        || raw.major > u8::MAX.into()
        || raw.minor > u8::MAX.into()
        || raw.flags > u8::MAX.into()
        || raw.reserved != 0
        || raw.control_known > 1
        || raw.range_known > 1
    {
        return Err(InvalidSnapshot);
    }
    let operation = match (raw.major, raw.minor) {
        (3, 0) => Operation::Read,
        (4, 0) => Operation::Write,
        (9, 0) => Operation::Flush,
        _ => Operation::Other,
    };
    let control_code = if raw.control_known == 1 {
        if (raw.major, raw.minor) != (0xe, 0) {
            return Err(InvalidSnapshot);
        }
        Some(raw.control_code)
    } else {
        if raw.control_code != 0 {
            return Err(InvalidSnapshot);
        }
        None
    };
    let range = if raw.range_known == 1 {
        if !matches!(operation, Operation::Read | Operation::Write)
            || raw.offset > i64::MAX as u64
            || raw.length > u32::MAX.into()
            || raw.offset.checked_add(raw.length).is_none()
        {
            return Err(InvalidSnapshot);
        }
        Some(RangeCandidate {
            offset: raw.offset,
            length: raw.length,
        })
    } else {
        if raw.offset != 0 || raw.length != 0 {
            return Err(InvalidSnapshot);
        }
        None
    };
    Ok(Submission {
        major: raw.major as u8,
        minor: raw.minor as u8,
        flags: raw.flags as u8,
        operation,
        range,
        control_code,
    })
}

pub fn decode_completion(
    raw: CompletionSnapshot,
    saved: &Submission,
) -> Result<Completion, SnapshotError> {
    use SnapshotError::InvalidSnapshot;
    if raw.version != 1 || raw.reserved != 0 || raw.information_known > 1 {
        return Err(InvalidSnapshot);
    }
    let information = if raw.information_known == 1 {
        if raw.status != 0 || !matches!(saved.operation, Operation::Read | Operation::Write) {
            return Err(InvalidSnapshot);
        }
        Some(raw.information)
    } else {
        if raw.information != 0 {
            return Err(InvalidSnapshot);
        }
        None
    };
    Ok(Completion {
        status: raw.status,
        information,
    })
}
