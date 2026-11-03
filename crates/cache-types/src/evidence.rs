//! Payload-free replay reports. Shape/arithmetic validation is not provenance or qualification.
use crate::wire::{NullableString, Object, UnsignedToken, decimal_u64};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

pub const EVIDENCE_MAX_BYTES: usize = 65_536;

macro_rules! vocabulary {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
        }
        impl std::str::FromStr for $name {
            type Err = EvidenceError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s { $($wire => Ok(Self::$variant)),+, _ => Err(EvidenceError::InvalidValue) }
            }
        }
    };
}

vocabulary!(Fault { None => "none", StaleFill => "stale_fill", EarlyAck => "early_ack", LateSlotReuse => "late_slot_reuse" });
vocabulary!(Outcome { Passed => "passed", Failed => "failed", Inconclusive => "inconclusive" });
vocabulary!(Terminal { Drained => "drained", Pending => "pending", PowerLoss => "power_loss", Stopped => "stopped" });
vocabulary!(ViolationKind { SemanticError => "semantic_error", BackendViolation => "backend_violation", SafetyViolation => "safety_violation", AssertionFailure => "assertion_failure" });
vocabulary!(ViolationCode {
    DuplicateCompletion => "DUPLICATE_COMPLETION", TransferAfterCompletion => "TRANSFER_AFTER_COMPLETION",
    TransferOverlap => "TRANSFER_OVERLAP", UnacceptedIo => "UNACCEPTED_IO", InvalidPhase => "INVALID_PHASE",
    PostPowerLoss => "POST_POWER_LOSS", ReadReportedUnsampled => "READ_REPORTED_UNSAMPLED",
    WriteReportedUnapplied => "WRITE_REPORTED_UNAPPLIED", FuaNotDurable => "FUA_NOT_DURABLE",
    FlushNotDurable => "FLUSH_NOT_DURABLE", DurableHistoryMismatch => "DURABLE_HISTORY_MISMATCH", StalePublication => "STALE_PUBLICATION",
    StaleCacheResult => "STALE_CACHE_RESULT", EarlyAck => "EARLY_ACK", LowerResultChanged => "LOWER_RESULT_CHANGED",
    LiveLeaseReused => "LIVE_LEASE_REUSED", ResultMutated => "RESULT_MUTATED", DuplicateResult => "DUPLICATE_RESULT",
    InvalidOwnership => "INVALID_OWNERSHIP", AssertRead => "ASSERT_READ", AssertResult => "ASSERT_RESULT",
    AssertCache => "ASSERT_CACHE", AssertDecision => "ASSERT_DECISION", AssertOrigin => "ASSERT_ORIGIN"
});

impl ViolationCode {
    pub const fn kind(self) -> ViolationKind {
        use ViolationCode::*;
        match self {
            DuplicateCompletion
            | TransferAfterCompletion
            | TransferOverlap
            | UnacceptedIo
            | InvalidPhase
            | PostPowerLoss => ViolationKind::SemanticError,
            ReadReportedUnsampled
            | WriteReportedUnapplied
            | FuaNotDurable
            | FlushNotDurable
            | DurableHistoryMismatch => ViolationKind::BackendViolation,
            StalePublication | StaleCacheResult | EarlyAck | LowerResultChanged
            | LiveLeaseReused | ResultMutated | DuplicateResult | InvalidOwnership => {
                ViolationKind::SafetyViolation
            }
            AssertRead | AssertResult | AssertCache | AssertDecision | AssertOrigin => {
                ViolationKind::AssertionFailure
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    OriginResults,
    CacheLineage,
    Ownership,
    Durability,
    TraceAssertions,
    Shadow,
}

impl Category {
    pub const ALL: [Self; 6] = [
        Self::OriginResults,
        Self::CacheLineage,
        Self::Ownership,
        Self::Durability,
        Self::TraceAssertions,
        Self::Shadow,
    ];
    pub const fn index(self) -> usize {
        self as usize
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OriginResults => "origin_results",
            Self::CacheLineage => "cache_lineage",
            Self::Ownership => "ownership",
            Self::Durability => "durability",
            Self::TraceAssertions => "trace_assertions",
            Self::Shadow => "shadow",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Check {
    pub passed: u64,
    pub failed: u64,
}

impl Check {
    pub const fn status(self, category: Category) -> &'static str {
        if matches!(category, Category::Shadow) {
            "not_applicable"
        } else if self.failed != 0 {
            "failed"
        } else if self.passed != 0 {
            "passed"
        } else {
            "not_observed"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub code: ViolationCode,
    pub event_index: u64,
}

/// Public values are revalidated on encoding. Hex shape does not authenticate provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub fault: Fault,
    pub seed: u64,
    pub source_revision: Option<String>,
    pub trace_digest: Option<String>,
    pub events_total: u64,
    pub events_executed: u64,
    pub outcome: Outcome,
    pub terminal: Terminal,
    pub pending_requests: u64,
    pub pending_ios: u64,
    pub abandoned_requests: u64,
    pub abandoned_ios: u64,
    pub violation: Option<Violation>,
    pub checks: [Check; 6],
}

impl Evidence {
    /// Saturates only for invalid, manually constructed values; encoding rejects overflow.
    pub fn meaningful_observations(&self) -> u64 {
        self.checks[..4].iter().fold(0_u64, |sum, c| {
            sum.saturating_add(c.passed).saturating_add(c.failed)
        })
    }
    pub fn assertions_checked(&self) -> u64 {
        let c = self.checks[Category::TraceAssertions.index()];
        c.passed.saturating_add(c.failed)
    }
    pub const fn mode(&self) -> &'static str {
        if matches!(self.fault, Fault::None) {
            "reference"
        } else {
            "mutant"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceError {
    InputTooLarge,
    InvalidJson,
    InvalidSchema,
    InvalidValue,
    InconsistentEvidence,
}
impl EvidenceError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::InvalidJson => "INVALID_JSON",
            Self::InvalidSchema => "INVALID_SCHEMA",
            Self::InvalidValue => "INVALID_VALUE",
            Self::InconsistentEvidence => "INCONSISTENT_EVIDENCE",
        }
    }
}
impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for EvidenceError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReport {
    schema_version: UnsignedToken,
    trace_schema_version: UnsignedToken,
    oracle_contract_version: UnsignedToken,
    mode: String,
    fault: String,
    seed: String,
    source_revision: NullableString,
    trace_digest: NullableString,
    events_total: String,
    events_executed: String,
    outcome: String,
    terminal: String,
    meaningful_observations: String,
    assertions_checked: String,
    pending_requests: String,
    pending_ios: String,
    abandoned_requests: String,
    abandoned_ios: String,
    violation: RequiredViolation,
    checks: Object<RawChecks>,
    kernel_runtime_tested: bool,
    production_qualified: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChecks {
    origin_results: Object<RawCheck>,
    cache_lineage: Object<RawCheck>,
    ownership: Object<RawCheck>,
    durability: Object<RawCheck>,
    trace_assertions: Object<RawCheck>,
    shadow: Object<RawCheck>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCheck {
    status: String,
    passed: String,
    failed: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawViolation {
    kind: String,
    code: String,
    event_index: String,
}
struct RequiredViolation(Option<RawViolation>);
impl<'de> Deserialize<'de> for RequiredViolation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Nullable;
        impl<'de> Visitor<'de> for Nullable {
            type Value = RequiredViolation;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("explicit null or violation object")
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(RequiredViolation(None))
            }
            fn visit_map<M: MapAccess<'de>>(self, m: M) -> Result<Self::Value, M::Error> {
                RawViolation::deserialize(MapAccessDeserializer::new(m))
                    .map(|v| RequiredViolation(Some(v)))
            }
        }
        // Not deserialize_option: missing required fields must not become None.
        d.deserialize_any(Nullable)
    }
}

fn number(s: &str) -> Result<u64, EvidenceError> {
    decimal_u64(s).ok_or(EvidenceError::InvalidValue)
}

pub fn parse_evidence(input: &str) -> Result<Evidence, EvidenceError> {
    if input.len() > EVIDENCE_MAX_BYTES {
        return Err(EvidenceError::InputTooLarge);
    }
    let Object(raw): Object<RawReport> = serde_json::from_str(input).map_err(|e| {
        if e.is_data() {
            EvidenceError::InvalidSchema
        } else {
            EvidenceError::InvalidJson
        }
    })?;
    if raw.schema_version.0 != 1
        || raw.trace_schema_version.0 != 1
        || raw.oracle_contract_version.0 != 1
        || raw.kernel_runtime_tested
        || raw.production_qualified
    {
        return Err(EvidenceError::InvalidSchema);
    }
    let c = raw.checks.0;
    let raw_checks = [
        c.origin_results.0,
        c.cache_lineage.0,
        c.ownership.0,
        c.durability.0,
        c.trace_assertions.0,
        c.shadow.0,
    ];
    let mut checks = [Check::default(); 6];
    for (category, raw_check) in Category::ALL.into_iter().zip(raw_checks) {
        let check = Check {
            passed: number(&raw_check.passed)?,
            failed: number(&raw_check.failed)?,
        };
        if raw_check.status != check.status(category) {
            return Err(EvidenceError::InconsistentEvidence);
        }
        checks[category.index()] = check;
    }
    let violation = raw
        .violation
        .0
        .map(|v| {
            let code: ViolationCode = v.code.parse()?;
            if v.kind != code.kind().as_str() {
                return Err(EvidenceError::InconsistentEvidence);
            }
            Ok(Violation {
                code,
                event_index: number(&v.event_index)?,
            })
        })
        .transpose()?;
    let report = Evidence {
        fault: raw.fault.parse()?,
        seed: number(&raw.seed)?,
        source_revision: raw.source_revision.0,
        trace_digest: raw.trace_digest.0,
        events_total: number(&raw.events_total)?,
        events_executed: number(&raw.events_executed)?,
        outcome: raw.outcome.parse()?,
        terminal: raw.terminal.parse()?,
        pending_requests: number(&raw.pending_requests)?,
        pending_ios: number(&raw.pending_ios)?,
        abandoned_requests: number(&raw.abandoned_requests)?,
        abandoned_ios: number(&raw.abandoned_ios)?,
        violation,
        checks,
    };
    validate(&report)?;
    if raw.mode != report.mode()
        || number(&raw.meaningful_observations)? != report.meaningful_observations()
        || number(&raw.assertions_checked)? != report.assertions_checked()
    {
        return Err(EvidenceError::InconsistentEvidence);
    }
    Ok(report)
}

fn validate(r: &Evidence) -> Result<(), EvidenceError> {
    let bad = EvidenceError::InconsistentEvidence;
    for (value, size) in [(&r.source_revision, 40), (&r.trace_digest, 64)] {
        if let Some(s) = value
            && (s.len() != size
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            return Err(EvidenceError::InvalidValue);
        }
    }
    if r.events_executed > r.events_total || r.events_total > 10_000 {
        return Err(bad);
    }
    if [
        r.pending_requests,
        r.pending_ios,
        r.abandoned_requests,
        r.abandoned_ios,
    ]
    .iter()
    .any(|v| *v > 1024)
    {
        return Err(bad);
    }
    let pending = r.pending_requests != 0 || r.pending_ios != 0;
    let abandoned = r.abandoned_requests != 0 || r.abandoned_ios != 0;
    if pending && abandoned {
        return Err(bad);
    }
    match r.terminal {
        Terminal::Drained if pending || abandoned => return Err(bad),
        Terminal::Pending if !pending || abandoned => return Err(bad),
        Terminal::PowerLoss if pending => return Err(bad),
        _ => {}
    }
    let mut failed = 0_u64;
    let mut meaningful = 0_u64;
    for category in Category::ALL {
        let c = r.checks[category.index()];
        let count = c.passed.checked_add(c.failed).ok_or(bad)?;
        let max = match category {
            Category::Ownership => 2 * r.events_executed,
            Category::Shadow => 0,
            _ => r.events_executed,
        };
        if count > max {
            return Err(bad);
        }
        failed = failed.checked_add(c.failed).ok_or(bad)?;
        if category.index() < 4 {
            meaningful = meaningful.checked_add(count).ok_or(bad)?;
        }
    }
    if let Some(v) = &r.violation {
        if r.outcome != Outcome::Failed
            || r.terminal != Terminal::Stopped
            || v.event_index.checked_add(1) != Some(r.events_executed)
            || r.events_executed == 0
        {
            return Err(bad);
        }
        let category_failed = |c: Category| r.checks[c.index()].failed == 1;
        let valid_failure = match v.code.kind() {
            ViolationKind::SemanticError => failed == 0,
            ViolationKind::AssertionFailure => {
                failed == 1 && category_failed(Category::TraceAssertions)
            }
            ViolationKind::BackendViolation => {
                failed == 1
                    && category_failed(match v.code {
                        ViolationCode::FuaNotDurable
                        | ViolationCode::FlushNotDurable
                        | ViolationCode::DurableHistoryMismatch => Category::Durability,
                        _ => Category::OriginResults,
                    })
            }
            ViolationKind::SafetyViolation => {
                failed == 1
                    && (category_failed(Category::OriginResults)
                        || category_failed(Category::Ownership)
                        || category_failed(Category::CacheLineage))
            }
        };
        if !valid_failure {
            return Err(bad);
        }
    } else {
        if r.events_executed != r.events_total || r.terminal == Terminal::Stopped || failed != 0 {
            return Err(bad);
        }
        let outcome = if pending || meaningful == 0 {
            Outcome::Inconclusive
        } else {
            Outcome::Passed
        };
        if r.outcome != outcome {
            return Err(bad);
        }
    }
    Ok(())
}

pub fn encode_evidence(r: &Evidence) -> Result<String, EvidenceError> {
    validate(r)?;
    // All strings below are fixed vocabulary or validated lowercase hex. Never payload/debug output.
    let optional = |s: &Option<String>| {
        s.as_ref()
            .map_or_else(|| "null".to_owned(), |s| format!("\"{s}\""))
    };
    let violation = r.violation.as_ref().map_or_else(
        || "null".to_owned(),
        |v| {
            format!(
                "{{\"kind\":\"{}\",\"code\":\"{}\",\"event_index\":\"{}\"}}",
                v.code.kind().as_str(),
                v.code.as_str(),
                v.event_index
            )
        },
    );
    let checks = Category::ALL
        .into_iter()
        .map(|category| {
            let c = r.checks[category.index()];
            format!(
                "\"{}\":{{\"status\":\"{}\",\"passed\":\"{}\",\"failed\":\"{}\"}}",
                category.as_str(),
                c.status(category),
                c.passed,
                c.failed
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        concat!(
            "{{\"schema_version\":1,\"trace_schema_version\":1,\"oracle_contract_version\":1,",
            "\"mode\":\"{}\",\"fault\":\"{}\",\"seed\":\"{}\",\"source_revision\":{},\"trace_digest\":{},",
            "\"events_total\":\"{}\",\"events_executed\":\"{}\",\"outcome\":\"{}\",\"terminal\":\"{}\",",
            "\"meaningful_observations\":\"{}\",\"assertions_checked\":\"{}\",",
            "\"pending_requests\":\"{}\",\"pending_ios\":\"{}\",\"abandoned_requests\":\"{}\",\"abandoned_ios\":\"{}\",",
            "\"violation\":{},\"checks\":{{{}}},\"kernel_runtime_tested\":false,\"production_qualified\":false}}\n"
        ),
        r.mode(),
        r.fault.as_str(),
        r.seed,
        optional(&r.source_revision),
        optional(&r.trace_digest),
        r.events_total,
        r.events_executed,
        r.outcome.as_str(),
        r.terminal.as_str(),
        r.meaningful_observations(),
        r.assertions_checked(),
        r.pending_requests,
        r.pending_ios,
        r.abandoned_requests,
        r.abandoned_ios,
        violation,
        checks
    ))
}
