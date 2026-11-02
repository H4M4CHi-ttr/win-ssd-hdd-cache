//! Strict offline status format; not a driver or service wire protocol.
use crate::wire::{NullableString, Object, UnsignedToken, json_optional_u64, nullable_decimal};
use serde::Deserialize;
use std::fmt;

pub const STATUS_MAX_BYTES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Text,
}

pub fn render_status(snapshot: &StatusSnapshot, format: OutputFormat) -> String {
    if format == OutputFormat::Json {
        return encode_status(snapshot);
    }
    let counter = |v: Option<u64>| v.map_or_else(|| "unavailable".to_owned(), |n| n.to_string());
    let code = snapshot.error_code.map_or("none", StatusErrorCode::as_str);
    let description = match snapshot.error_code {
        None => "none",
        Some(StatusErrorCode::CacheIo) => "Cache I/O failed.",
        Some(StatusErrorCode::CacheCorrupt) => "Cache data failed validation.",
        Some(StatusErrorCode::TargetChanged) => "Target identity changed.",
        Some(StatusErrorCode::Unsupported) => "Configuration is unsupported.",
        Some(StatusErrorCode::ResourceLimit) => "A resource limit was reached.",
        Some(StatusErrorCode::QuiescePending) => "Cache shutdown is still pending.",
    };
    let c = &snapshot.counters;
    format!(
        "state: {}\nmode: {}\nread_hits: {}\nread_misses: {}\norigin_read_bytes: {}\ncache_written_bytes: {}\nerror_code: {code}\nerror_description: {description}\n",
        snapshot.state.as_str(),
        snapshot.mode.as_str(),
        counter(c.read_hits),
        counter(c.read_misses),
        counter(c.origin_read_bytes),
        counter(c.cache_written_bytes)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusState {
    Disabled,
    Validating,
    Shadow,
    Active,
    Quiescing,
    Degraded,
    Removing,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusMode {
    ReadCacheWi,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusErrorCode {
    CacheIo,
    CacheCorrupt,
    TargetChanged,
    Unsupported,
    ResourceLimit,
    QuiescePending,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusCounters {
    pub read_hits: Option<u64>,
    pub read_misses: Option<u64>,
    pub origin_read_bytes: Option<u64>,
    pub cache_written_bytes: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub state: StatusState,
    pub mode: StatusMode,
    pub counters: StatusCounters,
    pub error_code: Option<StatusErrorCode>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusError {
    InputTooLarge,
    InvalidJson,
    InvalidSchema,
    InvalidValue,
}

impl StatusError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::InvalidJson => "INVALID_JSON",
            Self::InvalidSchema => "INVALID_SCHEMA",
            Self::InvalidValue => "INVALID_VALUE",
        }
    }
}
impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for StatusError {}

pub fn parse_status(input: &str) -> Result<StatusSnapshot, StatusError> {
    if input.len() > STATUS_MAX_BYTES {
        return Err(StatusError::InputTooLarge);
    }
    let Object(raw): Object<RawStatus> = serde_json::from_str(input).map_err(|error| {
        if error.is_data() {
            StatusError::InvalidSchema
        } else {
            StatusError::InvalidJson
        }
    })?;
    if raw.schema_version.0 != 1 {
        return Err(StatusError::InvalidSchema);
    }
    let state = match raw.state.as_str() {
        "disabled" => StatusState::Disabled,
        "validating" => StatusState::Validating,
        "shadow" => StatusState::Shadow,
        "active" => StatusState::Active,
        "quiescing" => StatusState::Quiescing,
        "degraded" => StatusState::Degraded,
        "removing" => StatusState::Removing,
        _ => return Err(StatusError::InvalidValue),
    };
    if raw.mode != "read_cache_wi" {
        return Err(StatusError::InvalidValue);
    }
    let c = raw.counters.0;
    let counter = |v| nullable_decimal(v).map_err(|()| StatusError::InvalidValue);
    let counters = StatusCounters {
        read_hits: counter(c.read_hits)?,
        read_misses: counter(c.read_misses)?,
        origin_read_bytes: counter(c.origin_read_bytes)?,
        cache_written_bytes: counter(c.cache_written_bytes)?,
    };
    let error_code = match raw.error_code.0.as_deref() {
        None => None,
        Some("CACHE_IO") => Some(StatusErrorCode::CacheIo),
        Some("CACHE_CORRUPT") => Some(StatusErrorCode::CacheCorrupt),
        Some("TARGET_CHANGED") => Some(StatusErrorCode::TargetChanged),
        Some("UNSUPPORTED") => Some(StatusErrorCode::Unsupported),
        Some("RESOURCE_LIMIT") => Some(StatusErrorCode::ResourceLimit),
        Some("QUIESCE_PENDING") => Some(StatusErrorCode::QuiescePending),
        Some(_) => return Err(StatusError::InvalidValue),
    };
    Ok(StatusSnapshot {
        state,
        mode: StatusMode::ReadCacheWi,
        counters,
        error_code,
    })
}
pub fn encode_status(snapshot: &StatusSnapshot) -> String {
    let c = &snapshot.counters;
    let error = snapshot
        .error_code
        .map_or_else(|| "null".to_owned(), |e| format!("\"{}\"", e.as_str()));
    // Every non-numeric value is a fixed enum spelling, never input text.
    format!(
        "{{\"schema_version\":1,\"state\":\"{}\",\"mode\":\"{}\",\"counters\":{{\"read_hits\":{},\"read_misses\":{},\"origin_read_bytes\":{},\"cache_written_bytes\":{}}},\"error_code\":{error}}}\n",
        snapshot.state.as_str(),
        snapshot.mode.as_str(),
        json_optional_u64(c.read_hits),
        json_optional_u64(c.read_misses),
        json_optional_u64(c.origin_read_bytes),
        json_optional_u64(c.cache_written_bytes),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStatus {
    schema_version: UnsignedToken,
    state: String,
    mode: String,
    counters: Object<RawCounters>,
    error_code: NullableString,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCounters {
    read_hits: NullableString,
    read_misses: NullableString,
    origin_read_bytes: NullableString,
    cache_written_bytes: NullableString,
}

impl StatusState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Validating => "validating",
            Self::Shadow => "shadow",
            Self::Active => "active",
            Self::Quiescing => "quiescing",
            Self::Degraded => "degraded",
            Self::Removing => "removing",
        }
    }
}
impl StatusMode {
    pub const fn as_str(self) -> &'static str {
        "read_cache_wi"
    }
}
impl StatusErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CacheIo => "CACHE_IO",
            Self::CacheCorrupt => "CACHE_CORRUPT",
            Self::TargetChanged => "TARGET_CHANGED",
            Self::Unsupported => "UNSUPPORTED",
            Self::ResourceLimit => "RESOURCE_LIMIT",
            Self::QuiescePending => "QUIESCE_PENDING",
        }
    }
}
