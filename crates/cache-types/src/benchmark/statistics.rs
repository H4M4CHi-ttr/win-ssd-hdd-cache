#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatencySummary {
    pub count: usize,
    pub minimum_ns: u64,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub maximum_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsError {
    Empty,
}

pub fn summarize_ns(samples: &[u64]) -> Result<LatencySummary, StatsError> {
    if samples.is_empty() {
        return Err(StatsError::Empty);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    // ceil(n*p/100), without multiplying n by p. Each term and their
    // sum are <= n for p <= 100; n > 0 and all three p are positive.
    let rank = |p: usize| (n / 100) * p + ((n % 100) * p).div_ceil(100) - 1;
    Ok(LatencySummary {
        count: n,
        minimum_ns: sorted[0],
        p50_ns: sorted[rank(50)],
        p95_ns: sorted[rank(95)],
        p99_ns: sorted[rank(99)],
        maximum_ns: sorted[n - 1],
    })
}
