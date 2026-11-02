//! Offline observations and deterministic statistics, not a measurement engine.
mod format;
mod statistics;

pub use format::{
    BENCHMARK_MAX_BYTES, BenchmarkCacheMode, BenchmarkError, BenchmarkRun, OsCache, SsdCache,
    encode_benchmark, parse_benchmark,
};
pub use statistics::{LatencySummary, StatsError, summarize_ns};
