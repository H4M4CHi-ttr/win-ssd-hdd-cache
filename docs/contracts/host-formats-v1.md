# Portable host formats and CLI v1

Normative amendment for C01/C02/C03/B01/B02, approved as part of AUD01. These are offline formats, not kernel or service protocols. Main contracts §§6/7 retain all field names and limits.

## 1. Strict input and non-reflecting errors

- Check UTF-8 byte length before parsing: status 65,536; benchmark 16,777,216. File readers read at most limit+1 bytes. No unbounded read-to-string.
- Exactly one top-level object. JSON whitespace around it is allowed; BOM, trailing documents/non-whitespace, top-level arrays and nested arrays in place of objects are rejected.
- Every key is required, including nullable keys. Missing is not explicit null. Unknown and duplicate keys are rejected at every object level, including escaped equivalent key spellings.
- schema_version must be unsigned JSON integer token 1, not 1.0, 1e0, a string, negative, bool or null. Repetitions is an unsigned integer token in 1..=1000.
- u64 strings are canonical ASCII decimal: 0 or [1-9][0-9]*, within u64 range. No signs, leading zeros, whitespace, exponent, non-ASCII digits or overflow. working_set_bytes/duration_ns additionally reject zero.
- Nullable counters/metrics accept only explicit null or canonical u64 strings. Samples are independently 1..=100,000; their count need not equal repetitions.
- Return one deterministic error for a given input; no multi-error aggregation required. Size is checked first, structural decoding before semantic validation. Do not expose raw serde errors, unknown field names or payload text.

StatusError and BenchmarkError each have `InputTooLarge`, `InvalidJson`, `InvalidSchema`, `InvalidValue`. `code(&self) -> &'static str` and Display return respectively `INPUT_TOO_LARGE`, `INVALID_JSON`, `INVALID_SCHEMA`, `INVALID_VALUE`, with no payload. Syntax/EOF/trailing input is InvalidJson; structure/types/duplicate/missing/unknown/unsupported version is InvalidSchema; enum spellings/canonical integer/range/mode combinations are InvalidValue. Malformed input may stop at its first structural error.

## 2. Status types and codec (C01)

```rust
pub enum StatusState { Disabled, Validating, Shadow, Active, Quiescing, Degraded, Removing }
pub enum StatusMode { ReadCacheWi }
pub enum StatusErrorCode { CacheIo, CacheCorrupt, TargetChanged, Unsupported, ResourceLimit, QuiescePending }
pub struct StatusCounters {
    pub read_hits: Option<u64>, pub read_misses: Option<u64>,
    pub origin_read_bytes: Option<u64>, pub cache_written_bytes: Option<u64>,
}
pub struct StatusSnapshot {
    pub state: StatusState, pub mode: StatusMode,
    pub counters: StatusCounters, pub error_code: Option<StatusErrorCode>,
}
pub fn parse_status(input: &str) -> Result<StatusSnapshot, StatusError>;
pub fn encode_status(snapshot: &StatusSnapshot) -> String;
```

Types derive Debug/Clone/PartialEq/Eq; small enums additionally Copy. schema_version is codec-owned, not a mutable public field. Wire spellings stay exactly as in contracts §6. The encoder emits one compact JSON line with exactly one LF, in this order:

```json
{"schema_version":1,"state":"disabled","mode":"read_cache_wi","counters":{"read_hits":null,"read_misses":null,"origin_read_bytes":null,"cache_written_bytes":null},"error_code":null}
```

Non-null u64s remain quoted decimal strings. Explicit nullable presence must not be erased by serde Option defaults. Types contain no free-form error text.

## 3. Status rendering and command (C02)

`cache_types::status::OutputFormat` has Json/Text. `render_status(&StatusSnapshot, OutputFormat) -> String` is added in C02; Json delegates to encode_status. Text is exactly eight lines, each terminated by LF even on Windows:

```text
state: disabled
mode: read_cache_wi
read_hits: unavailable
read_misses: unavailable
origin_read_bytes: unavailable
cache_written_bytes: unavailable
error_code: none
error_description: none
```

Known counters replace unavailable with decimal digits. No banners, timestamps, ANSI or safety-guarantee wording. Fixed error descriptions:

| Code | Description |
|---|---|
| CACHE_IO | Cache I/O failed. |
| CACHE_CORRUPT | Cache data failed validation. |
| TARGET_CHANGED | Target identity changed. |
| UNSUPPORTED | Configuration is unsupported. |
| RESOURCE_LIMIT | A resource limit was reached. |
| QUIESCE_PENDING | Cache shutdown is still pending. |

Command: `cache-cli status --input <file> --format json|text`. Both flags are required once, either order. Reject unknown, repeated, missing and extra arguments; no stdin/live-service fallback. Paths are OS strings (Unicode/non-UTF8 where the host permits), not reflected into error messages.

| Outcome | Exit | stdout | stderr |
|---|---:|---|---|
| success | 0 | rendered output | empty |
| arguments | 2 | empty | `error: USAGE` + LF |
| invalid UTF-8 | 2 | empty | `error: INVALID_JSON` + LF |
| parse/value/size | 2 | empty | `error: <fixed-code>` + LF |
| file I/O | 3 | empty | `error: INPUT_IO` + LF |

Only regular input files are supported; reject directories/special files without reading them indefinitely. No network, raw-device, pipe, or service access is a feature of this CLI.

## 4. Public diagnostics (C03)

`PublicDiagnostics` is a separately owned allowlist type with state, mode, four optional u64 counters and optional fixed error code only. `public_diagnostics(&str) -> Result<PublicDiagnostics, StatusError>` first uses strict status parsing; it never holds the source string/unknown values or raw parser errors. Debug/Display/error outputs must not echo secret sentinel values from rejected input. Do not add identifiers, paths or free-form messages. This task does not add a CLI command.

## 5. Benchmark types and codec (B01)

```rust
pub enum OsCache { Cold, Warm, Unknown }
pub enum BenchmarkCacheMode { Off, Shadow, Active }
pub enum SsdCache { None, Cold, Warm }
pub struct BenchmarkRun {
    pub workload_id: String, pub environment_id: String, pub toolchain_id: String,
    pub working_set_bytes: u64, pub os_cache: OsCache,
    pub cache_mode: BenchmarkCacheMode, pub ssd_cache: SsdCache,
    pub duration_ns: u64, pub latency_ns: Vec<u64>,
    pub cpu_time_ns: Option<u64>, pub peak_nonpaged_bytes: Option<u64>,
    pub ssd_written_bytes: Option<u64>, pub repetitions: u16,
}
pub fn parse_benchmark(input: &str) -> Result<BenchmarkRun, BenchmarkError>;
pub fn encode_benchmark(run: &BenchmarkRun) -> Result<String, BenchmarkError>;
```

Types derive Debug/Clone/PartialEq/Eq; small enums Copy. The encoder revalidates public fields, emits compact JSON + LF with keys in the struct order above preceded by schema_version, and preserves nullable fields. The parser/encoder share validation, not the independent statistics test expectations. IDs keep contracts §7 ASCII pattern/length. No statistics are inferred from missing metrics.

## 6. Comparison (B02)

```rust
pub struct BenchmarkSummary {
    pub cache_mode: BenchmarkCacheMode, pub ssd_cache: SsdCache,
    pub duration_ns: u64, pub latency: LatencySummary,
}
pub struct BenchmarkComparison {
    pub base: BenchmarkSummary, pub candidate: BenchmarkSummary, pub speedup: f64,
}
pub enum CompareError {
    InvalidBase, InvalidCandidate,
    WorkloadId, EnvironmentId, ToolchainId, WorkingSetBytes, OsCache, Repetitions,
}
pub fn compare_runs(base: &BenchmarkRun, candidate: &BenchmarkRun)
    -> Result<BenchmarkComparison, CompareError>;
```

Revalidate base then candidate, including nonempty bounded samples and positive durations, even for manually constructed public structs. InvalidBase/InvalidCandidate precede field comparison; no panic/Infinity. `CompareError::field()` and Display return only fixed `base`, `candidate`, `workload_id`, `environment_id`, `toolchain_id`, `working_set_bytes`, `os_cache`, `repetitions`. Test equality in that order; OsCache also rejects either Unknown. Treatments cache_mode/ssd_cache may differ and must be shown. Stored speedup is base.duration_ns as f64 / candidate.duration_ns as f64, finite and positive, rounded only for display.

## 7. Report command and output (B02)

`cache-cli benchmark-report --base <file> --candidate <file> --format json|text`. All flags required once, arbitrary order. Base reads/parses first, candidate second, comparison third. Same bounded regular-file/UTF-8 rules as status. No input path or identifier values are printed.

Text has this exact order and a final LF; speedup always six fractional decimal digits (rounding may display a very small positive ratio as zero):

```text
base.cache_mode: off
base.ssd_cache: none
base.duration_ns: 1000000000
base.p95_ns: 100
base.p99_ns: 100
candidate.cache_mode: active
candidate.ssd_cache: warm
candidate.duration_ns: 800000000
candidate.p95_ns: 80
candidate.p99_ns: 80
speedup: 1.250000
```

JSON has one compact line + LF, this key order, quoted u64s and display-only speedup:

```json
{"schema_version":1,"base":{"cache_mode":"off","ssd_cache":"none","duration_ns":"1000000000","p95_ns":"100","p99_ns":"100"},"candidate":{"cache_mode":"active","ssd_cache":"warm","duration_ns":"800000000","p95_ns":"80","p99_ns":"80"},"speedup":"1.250000"}
```

Success exit 0, stderr empty. Failures stdout empty: arguments exit 2 `error: USAGE\n`; parse exit 2 `error: base: <fixed-code>\n` or candidate; file I/O exit 3 `error: base: INPUT_IO\n` or candidate; refused comparison exit 2 `error: compare: <fixed-field-name>\n`. These commands analyze supplied observations; they do not perform or certify measurements.
