//! Experimental, unauthenticated native probe archive support.
#![forbid(unsafe_code)]

mod archive;
mod project;
mod wire;

pub use archive::{
    Archive, CaptureBuilder, CollectorFault, NativeAudit, audit_archive, decode_archive,
};
pub use project::project_v2;
pub use wire::{Command, Frame, Observation, Record, Request, decode_response};

pub const MAX_ARCHIVE_BYTES: usize = 1_048_576;
pub const MAX_FRAMES: usize = 4096;
pub const MAX_RECORDS: usize = 4096;

/// Fixed, non-reflecting errors. They carry no source bytes or machine details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeError {
    InvalidArchive,
    TooLarge,
    ProjectionTooLarge,
}

impl ProbeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidArchive => "INVALID_PROBE_ARCHIVE",
            Self::TooLarge => "PROBE_TOO_LARGE",
            Self::ProjectionTooLarge => "PROBE_PROJECTION_TOO_LARGE",
        }
    }
}
