//! Public offline diagnostics deliberately have no identifier or free-text fields.
use crate::status::{StatusError, StatusErrorCode, StatusMode, StatusState, parse_status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicDiagnostics {
    pub state: StatusState,
    pub mode: StatusMode,
    pub read_hits: Option<u64>,
    pub read_misses: Option<u64>,
    pub origin_read_bytes: Option<u64>,
    pub cache_written_bytes: Option<u64>,
    pub error_code: Option<StatusErrorCode>,
}

pub fn public_diagnostics(input: &str) -> Result<PublicDiagnostics, StatusError> {
    let status = parse_status(input)?;
    Ok(PublicDiagnostics {
        state: status.state,
        mode: status.mode,
        read_hits: status.counters.read_hits,
        read_misses: status.counters.read_misses,
        origin_read_bytes: status.counters.origin_read_bytes,
        cache_written_bytes: status.counters.cache_written_bytes,
        error_code: status.error_code,
    })
}
