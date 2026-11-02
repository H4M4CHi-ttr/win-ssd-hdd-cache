//! Offline observations and deterministic statistics, not a measurement engine.
mod compare;
mod format;
mod statistics;

pub use compare::{BenchmarkComparison, BenchmarkSummary, CompareError, compare_runs};
pub use format::{
    BENCHMARK_MAX_BYTES, BenchmarkCacheMode, BenchmarkError, BenchmarkRun, OsCache, SsdCache,
    encode_benchmark, parse_benchmark,
};
pub use statistics::{LatencySummary, StatsError, summarize_ns};
