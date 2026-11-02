use crate::wire::{
    NullableString, Object, UnsignedToken, decimal_u64, json_optional_u64, nullable_decimal,
};
use serde::Deserialize;
use std::fmt;

pub const BENCHMARK_MAX_BYTES: usize = 16_777_216;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsCache {
    Cold,
    Warm,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkCacheMode {
    Off,
    Shadow,
    Active,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsdCache {
    None,
    Cold,
    Warm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkRun {
    pub workload_id: String,
    pub environment_id: String,
    pub toolchain_id: String,
    pub working_set_bytes: u64,
    pub os_cache: OsCache,
    pub cache_mode: BenchmarkCacheMode,
    pub ssd_cache: SsdCache,
    pub duration_ns: u64,
    pub latency_ns: Vec<u64>,
    pub cpu_time_ns: Option<u64>,
    pub peak_nonpaged_bytes: Option<u64>,
    pub ssd_written_bytes: Option<u64>,
    pub repetitions: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkError {
    InputTooLarge,
    InvalidJson,
    InvalidSchema,
    InvalidValue,
}
impl BenchmarkError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::InvalidJson => "INVALID_JSON",
            Self::InvalidSchema => "INVALID_SCHEMA",
            Self::InvalidValue => "INVALID_VALUE",
        }
    }
}
impl fmt::Display for BenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for BenchmarkError {}

pub fn parse_benchmark(input: &str) -> Result<BenchmarkRun, BenchmarkError> {
    if input.len() > BENCHMARK_MAX_BYTES {
        return Err(BenchmarkError::InputTooLarge);
    }
    let Object(raw): Object<RawBenchmark> = serde_json::from_str(input).map_err(|error| {
        if error.is_data() {
            BenchmarkError::InvalidSchema
        } else {
            BenchmarkError::InvalidJson
        }
    })?;
    if raw.schema_version.0 != 1 {
        return Err(BenchmarkError::InvalidSchema);
    }
    let os_cache = match raw.os_cache.as_str() {
        "cold" => OsCache::Cold,
        "warm" => OsCache::Warm,
        "unknown" => OsCache::Unknown,
        _ => return Err(BenchmarkError::InvalidValue),
    };
    let cache_mode = match raw.cache_mode.as_str() {
        "off" => BenchmarkCacheMode::Off,
        "shadow" => BenchmarkCacheMode::Shadow,
        "active" => BenchmarkCacheMode::Active,
        _ => return Err(BenchmarkError::InvalidValue),
    };
    let ssd_cache = match raw.ssd_cache.as_str() {
        "none" => SsdCache::None,
        "cold" => SsdCache::Cold,
        "warm" => SsdCache::Warm,
        _ => return Err(BenchmarkError::InvalidValue),
    };
    if raw.latency_ns.is_empty()
        || raw.latency_ns.len() > 100_000
        || !(1..=1000).contains(&raw.repetitions.0)
    {
        return Err(BenchmarkError::InvalidValue);
    }
    let decimal = |s: &str| decimal_u64(s).ok_or(BenchmarkError::InvalidValue);
    let nullable = |s| nullable_decimal(s).map_err(|()| BenchmarkError::InvalidValue);
    let run = BenchmarkRun {
        workload_id: raw.workload_id,
        environment_id: raw.environment_id,
        toolchain_id: raw.toolchain_id,
        working_set_bytes: decimal(&raw.working_set_bytes)?,
        os_cache,
        cache_mode,
        ssd_cache,
        duration_ns: decimal(&raw.duration_ns)?,
        latency_ns: raw
            .latency_ns
            .iter()
            .map(|s| decimal(s))
            .collect::<Result<_, _>>()?,
        cpu_time_ns: nullable(raw.cpu_time_ns)?,
        peak_nonpaged_bytes: nullable(raw.peak_nonpaged_bytes)?,
        ssd_written_bytes: nullable(raw.ssd_written_bytes)?,
        repetitions: raw.repetitions.0 as u16,
    };
    validate(&run)?;
    Ok(run)
}
pub fn encode_benchmark(run: &BenchmarkRun) -> Result<String, BenchmarkError> {
    validate(run)?;
    let samples = run
        .latency_ns
        .iter()
        .map(|n| format!("\"{n}\""))
        .collect::<Vec<_>>()
        .join(",");
    // IDs are validated ASCII identifiers; all other strings are fixed enum
    // spellings or decimal digits, so no unescaped arbitrary text is inserted.
    Ok(format!(
        "{{\"schema_version\":1,\"workload_id\":\"{}\",\"environment_id\":\"{}\",\"toolchain_id\":\"{}\",\"working_set_bytes\":\"{}\",\"os_cache\":\"{}\",\"cache_mode\":\"{}\",\"ssd_cache\":\"{}\",\"duration_ns\":\"{}\",\"latency_ns\":[{samples}],\"cpu_time_ns\":{},\"peak_nonpaged_bytes\":{},\"ssd_written_bytes\":{},\"repetitions\":{}}}\n",
        run.workload_id,
        run.environment_id,
        run.toolchain_id,
        run.working_set_bytes,
        run.os_cache.as_str(),
        run.cache_mode.as_str(),
        run.ssd_cache.as_str(),
        run.duration_ns,
        json_optional_u64(run.cpu_time_ns),
        json_optional_u64(run.peak_nonpaged_bytes),
        json_optional_u64(run.ssd_written_bytes),
        run.repetitions,
    ))
}

pub(super) fn validate(run: &BenchmarkRun) -> Result<(), BenchmarkError> {
    let valid_id = |s: &str| {
        (1..=64).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    let valid_pair = matches!(
        (run.cache_mode, run.ssd_cache),
        (BenchmarkCacheMode::Off, SsdCache::None)
            | (
                BenchmarkCacheMode::Shadow | BenchmarkCacheMode::Active,
                SsdCache::Cold | SsdCache::Warm
            )
    );
    if !valid_id(&run.workload_id)
        || !valid_id(&run.environment_id)
        || !valid_id(&run.toolchain_id)
        || run.working_set_bytes == 0
        || run.duration_ns == 0
        || !(1..=100_000).contains(&run.latency_ns.len())
        || !(1..=1000).contains(&run.repetitions)
        || !valid_pair
    {
        return Err(BenchmarkError::InvalidValue);
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBenchmark {
    schema_version: UnsignedToken,
    workload_id: String,
    environment_id: String,
    toolchain_id: String,
    working_set_bytes: String,
    os_cache: String,
    cache_mode: String,
    ssd_cache: String,
    duration_ns: String,
    latency_ns: Vec<String>,
    cpu_time_ns: NullableString,
    peak_nonpaged_bytes: NullableString,
    ssd_written_bytes: NullableString,
    repetitions: UnsignedToken,
}

impl OsCache {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
            Self::Unknown => "unknown",
        }
    }
}
impl BenchmarkCacheMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Shadow => "shadow",
            Self::Active => "active",
        }
    }
}
impl SsdCache {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Cold => "cold",
            Self::Warm => "warm",
        }
    }
}
