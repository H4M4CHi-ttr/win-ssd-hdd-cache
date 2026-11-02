use super::{
    BenchmarkCacheMode, BenchmarkRun, LatencySummary, OsCache, SsdCache, format::validate,
    summarize_ns,
};
use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkSummary {
    pub cache_mode: BenchmarkCacheMode,
    pub ssd_cache: SsdCache,
    pub duration_ns: u64,
    pub latency: LatencySummary,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkComparison {
    pub base: BenchmarkSummary,
    pub candidate: BenchmarkSummary,
    pub speedup: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareError {
    InvalidBase,
    InvalidCandidate,
    WorkloadId,
    EnvironmentId,
    ToolchainId,
    WorkingSetBytes,
    OsCache,
    Repetitions,
}
impl CompareError {
    pub const fn field(&self) -> &'static str {
        match self {
            Self::InvalidBase => "base",
            Self::InvalidCandidate => "candidate",
            Self::WorkloadId => "workload_id",
            Self::EnvironmentId => "environment_id",
            Self::ToolchainId => "toolchain_id",
            Self::WorkingSetBytes => "working_set_bytes",
            Self::OsCache => "os_cache",
            Self::Repetitions => "repetitions",
        }
    }
}
impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.field())
    }
}
impl std::error::Error for CompareError {}
pub fn compare_runs(
    base: &BenchmarkRun,
    candidate: &BenchmarkRun,
) -> Result<BenchmarkComparison, CompareError> {
    validate(base).map_err(|_| CompareError::InvalidBase)?;
    validate(candidate).map_err(|_| CompareError::InvalidCandidate)?;
    for (same, error) in [
        (
            base.workload_id == candidate.workload_id,
            CompareError::WorkloadId,
        ),
        (
            base.environment_id == candidate.environment_id,
            CompareError::EnvironmentId,
        ),
        (
            base.toolchain_id == candidate.toolchain_id,
            CompareError::ToolchainId,
        ),
        (
            base.working_set_bytes == candidate.working_set_bytes,
            CompareError::WorkingSetBytes,
        ),
        (
            base.os_cache == candidate.os_cache && base.os_cache != OsCache::Unknown,
            CompareError::OsCache,
        ),
        (
            base.repetitions == candidate.repetitions,
            CompareError::Repetitions,
        ),
    ] {
        if !same {
            return Err(error);
        }
    }
    let summary = |run: &BenchmarkRun, error| {
        Ok(BenchmarkSummary {
            cache_mode: run.cache_mode,
            ssd_cache: run.ssd_cache,
            duration_ns: run.duration_ns,
            latency: summarize_ns(&run.latency_ns).map_err(|_| error)?,
        })
    };
    Ok(BenchmarkComparison {
        base: summary(base, CompareError::InvalidBase)?,
        candidate: summary(candidate, CompareError::InvalidCandidate)?,
        speedup: base.duration_ns as f64 / candidate.duration_ns as f64,
    })
}
