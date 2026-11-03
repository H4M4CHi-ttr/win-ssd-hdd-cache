//! Bounded offline correlation audit. Internal completeness is not authenticity,
//! authorization, interception coverage or Windows/kernel qualification.
use crate::wire::{NullableString, Object, UnsignedToken, decimal_u64, nullable_decimal};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

pub const CAPTURE_MAX_BYTES: usize = 1_048_576;
pub const CAPTURE_MAX_RECORDS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    TooLarge,
    InvalidCapture,
    UnsupportedVersion,
    TooManyRecords,
    InvalidValue,
}

impl CaptureError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "CAPTURE_TOO_LARGE",
            Self::InvalidCapture => "INVALID_CAPTURE",
            Self::UnsupportedVersion => "UNSUPPORTED_CAPTURE_VERSION",
            Self::TooManyRecords => "TOO_MANY_CAPTURE_RECORDS",
            Self::InvalidValue => "INVALID_CAPTURE_VALUE",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCapture {
    schema_version: UnsignedToken,
    evidence_kind: String,
    observation_point: String,
    session_id: String,
    windows_version: NullableString,
    source_revision: NullableString,
    producer_sha256: NullableString,
    capture_finalized: bool,
    start_inflight: NullableString,
    end_inflight: NullableString,
    events_attempted: NullableString,
    events_dropped: NullableString,
    records: Vec<Object<RawRecord>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecord {
    seq: String,
    phase: String,
    request_id: String,
    target_id: String,
    operation: NullableString,
    offset: NullableString,
    length: NullableString,
    status: NullableString,
    information: NullableString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureVersion {
    V1,
    V2,
}

impl CaptureVersion {
    const fn number(self) -> u8 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }
}

#[derive(Debug)]
enum Phase {
    Submit { unknown_range: bool, other: bool },
    Complete { unknown_information: bool },
}

struct Record {
    seq: u64,
    request_id: u64,
    target_id: u64,
    phase: Phase,
}

// These positions are the contract's canonical report order, never event order.
const REASONS: [&str; 15] = [
    "EMPTY_CAPTURE",
    "NOT_FINALIZED",
    "UNKNOWN_COUNTERS",
    "BOUNDARY_INFLIGHT",
    "DROPPED_EVENTS",
    "SEQUENCE_ORDER",
    "SEQUENCE_GAP",
    "COUNT_MISMATCH",
    "DUPLICATE_REQUEST",
    "TARGET_MISMATCH",
    "UNPAIRED_REQUEST",
    "UNKNOWN_RANGE",
    "UNCLASSIFIED_OPERATION",
    "MISSING_PROVENANCE",
    "UNKNOWN_INFORMATION",
];

/// Only the audit can construct a verdict. No source IDs or free text escape in
/// the report, and every authenticity/authorization/qualification flag is false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureAudit {
    version: CaptureVersion,
    evidence_kind: &'static str,
    observation_point: &'static str,
    records: u64,
    submits: u64,
    completions: u64,
    paired_requests: u64,
    pending_requests: u64,
    orphan_completions: u64,
    reasons: [bool; 15],
}

impl CaptureAudit {
    pub fn is_internally_complete(&self) -> bool {
        !self.reasons.iter().any(|set| *set)
    }

    pub fn to_json(&self) -> String {
        let reasons = REASONS
            .iter()
            .zip(self.reasons)
            .filter(|(_, set)| *set)
            .map(|(code, _)| format!("\"{code}\""))
            .collect::<Vec<_>>()
            .join(",");
        let verdict = if self.is_internally_complete() {
            "internally_complete"
        } else {
            "incomplete"
        };
        // All string substitutions are closed static vocabularies; source text
        // is never interpolated. Counts are bounded by the accepted record cap.
        format!(
            "{{\"schema_version\":{},\"evidence_kind\":\"{}\",\"observation_point\":\"{}\",\"verdict\":\"{}\",\"records\":\"{}\",\"submits\":\"{}\",\"completions\":\"{}\",\"paired_requests\":\"{}\",\"pending_requests\":\"{}\",\"orphan_completions\":\"{}\",\"reasons\":[{}],\"producer_verified\":false,\"authorization_verified\":false,\"coverage_verified\":false,\"kernel_runtime_qualified\":false}}\n",
            self.version.number(),
            self.evidence_kind,
            self.observation_point,
            verdict,
            self.records,
            self.submits,
            self.completions,
            self.paired_requests,
            self.pending_requests,
            self.orphan_completions,
            reasons,
        )
    }
}

fn decimal(value: &str) -> Result<u64, CaptureError> {
    decimal_u64(value).ok_or(CaptureError::InvalidValue)
}

fn positive(value: &str) -> Result<u64, CaptureError> {
    decimal(value).and_then(|n| {
        if n == 0 {
            Err(CaptureError::InvalidValue)
        } else {
            Ok(n)
        }
    })
}

fn optional(value: NullableString) -> Result<Option<u64>, CaptureError> {
    nullable_decimal(value).map_err(|()| CaptureError::InvalidValue)
}

fn check_hex(value: &NullableString, length: usize) -> Result<(), CaptureError> {
    if value.0.as_ref().is_some_and(|text| {
        text.len() != length
            || !text
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    }) {
        return Err(CaptureError::InvalidValue);
    }
    Ok(())
}

fn check_version(value: &NullableString) -> Result<(), CaptureError> {
    if let Some(text) = &value.0 {
        let parts: Vec<_> = text.split('.').collect();
        if parts.len() != 4
            || !parts
                .iter()
                .all(|part| decimal_u64(part).is_some_and(|n| u32::try_from(n).is_ok()))
        {
            return Err(CaptureError::InvalidValue);
        }
    }
    Ok(())
}

fn record(raw: RawRecord, version: CaptureVersion) -> Result<Record, CaptureError> {
    let seq = decimal(&raw.seq)?;
    if raw.phase != "submit" && raw.phase != "complete" {
        return Err(CaptureError::InvalidValue);
    }
    let request_id = positive(&raw.request_id)?;
    let target_id = positive(&raw.target_id)?;
    let operation = raw.operation.0.as_deref();
    let offset = optional(raw.offset)?;
    let length = optional(raw.length)?;
    let status = optional(raw.status)?;
    let information = optional(raw.information)?;
    let phase = if raw.phase == "submit" {
        if status.is_some() || information.is_some() {
            return Err(CaptureError::InvalidValue);
        }
        match operation {
            Some("read" | "write") => {
                match (offset, length) {
                    (Some(start), Some(len)) if start.checked_add(len).is_some() => (),
                    (None, None) => (),
                    _ => return Err(CaptureError::InvalidValue),
                }
                Phase::Submit {
                    unknown_range: offset.is_none(),
                    other: false,
                }
            }
            Some("flush" | "other") if offset.is_none() && length.is_none() => Phase::Submit {
                unknown_range: false,
                other: operation == Some("other"),
            },
            _ => return Err(CaptureError::InvalidValue),
        }
    } else {
        if operation.is_some()
            || offset.is_some()
            || length.is_some()
            || !status.is_some_and(|n| u32::try_from(n).is_ok())
            || (version == CaptureVersion::V1 && information.is_none())
        {
            return Err(CaptureError::InvalidValue);
        }
        // Completion status/information are observations, not a portable I/O
        // success/transfer rule. Validate their representation, never normalize.
        Phase::Complete {
            unknown_information: information.is_none(),
        }
    };
    Ok(Record {
        seq,
        request_id,
        target_id,
        phase,
    })
}

struct Submission {
    target: u64,
    paired: bool,
}

/// Checks the supplied projection only. It cannot establish truthful capture,
/// complete interception, the target's address mapping or permission to operate.
pub fn audit_capture(input: &str) -> Result<CaptureAudit, CaptureError> {
    if input.len() > CAPTURE_MAX_BYTES {
        return Err(CaptureError::TooLarge);
    }
    // Object + required NullableString retain key/shape evidence. Never parse
    // through serde_json::Value, which would erase duplicate names.
    let Object(raw): Object<RawCapture> =
        serde_json::from_str(input).map_err(|_| CaptureError::InvalidCapture)?;
    let version = match raw.schema_version.0 {
        1 => CaptureVersion::V1,
        2 => CaptureVersion::V2,
        _ => return Err(CaptureError::UnsupportedVersion),
    };
    if raw.records.len() > CAPTURE_MAX_RECORDS {
        return Err(CaptureError::TooManyRecords);
    }
    let evidence_kind = match raw.evidence_kind.as_str() {
        "synthetic" => "synthetic",
        "windows_observation" => "windows_observation",
        _ => return Err(CaptureError::InvalidValue),
    };
    let observation_point = match raw.observation_point.as_str() {
        "disk_class_upper" => "disk_class_upper",
        "disk_class_lower_srb" => "disk_class_lower_srb",
        _ => return Err(CaptureError::InvalidValue),
    };
    positive(&raw.session_id)?;
    check_version(&raw.windows_version)?;
    check_hex(&raw.source_revision, 40)?;
    check_hex(&raw.producer_sha256, 64)?;
    let missing_provenance = evidence_kind == "windows_observation"
        && (raw.windows_version.0.is_none()
            || raw.source_revision.0.is_none()
            || raw.producer_sha256.0.is_none());
    let start = optional(raw.start_inflight)?;
    let end = optional(raw.end_inflight)?;
    let attempted = optional(raw.events_attempted)?;
    let dropped = optional(raw.events_dropped)?;
    let records: Vec<_> = raw
        .records
        .into_iter()
        .map(|Object(r)| record(r, version))
        .collect::<Result<_, _>>()?;
    // Only validated, bounded records reach correlation-state allocation.
    let mut result = CaptureAudit {
        version,
        evidence_kind,
        observation_point,
        records: records.len() as u64,
        submits: 0,
        completions: 0,
        paired_requests: 0,
        pending_requests: 0,
        orphan_completions: 0,
        reasons: [false; 15],
    };
    result.reasons[0] = records.is_empty();
    result.reasons[1] = !raw.capture_finalized;
    result.reasons[2] = [start, end, attempted, dropped].contains(&None);
    result.reasons[3] = start.is_some_and(|n| n > 0) || end.is_some_and(|n| n > 0);
    result.reasons[4] = dropped.is_some_and(|n| n > 0);
    result.reasons[13] = missing_provenance;
    if let (Some(attempted), Some(dropped)) = (attempted, dropped) {
        result.reasons[7] = result.records.checked_add(dropped) != Some(attempted);
    }
    let mut previous: Option<u64> = None;
    let mut submissions: BTreeMap<u64, Submission> = BTreeMap::new();
    let mut completions = BTreeSet::new();
    for r in records {
        if let Some(prev) = previous {
            result.reasons[5] |= r.seq <= prev;
            result.reasons[6] |= prev.checked_add(1) != Some(r.seq);
        } else {
            result.reasons[6] |= r.seq != 0;
        }
        previous = Some(r.seq);
        match r.phase {
            Phase::Submit {
                unknown_range,
                other,
            } => {
                result.submits += 1;
                result.reasons[11] |= unknown_range;
                result.reasons[12] |= other;
                match submissions.entry(r.request_id) {
                    Entry::Vacant(entry) => {
                        entry.insert(Submission {
                            target: r.target_id,
                            paired: false,
                        });
                    }
                    Entry::Occupied(_) => {
                        result.reasons[8] = true;
                    }
                }
            }
            Phase::Complete {
                unknown_information,
            } => {
                result.completions += 1;
                // Every supplied completion carries its own uncertainty, even
                // when a duplicate cannot participate in correlation again.
                result.reasons[14] |= unknown_information;
                // Reserve first occurrence, including an orphan/mismatch. A
                // duplicate is never a repair or a second successful pair.
                if !completions.insert(r.request_id) {
                    result.reasons[8] = true;
                    continue;
                }
                match submissions.get_mut(&r.request_id) {
                    None => result.orphan_completions += 1,
                    Some(s) if s.target != r.target_id => result.reasons[9] = true,
                    Some(s) if !s.paired => {
                        s.paired = true;
                        result.paired_requests += 1;
                    }
                    Some(_) => result.reasons[8] = true,
                }
            }
        }
    }
    match previous {
        Some(last) => match last.checked_add(1) {
            None => result.reasons[6] = true,
            Some(next) => result.reasons[6] |= attempted.is_some_and(|n| n != next),
        },
        None => result.reasons[6] |= attempted.is_some_and(|n| n > 0),
    }
    result.pending_requests = submissions.values().filter(|s| !s.paired).count() as u64;
    result.reasons[10] = result.pending_requests > 0 || result.orphan_completions > 0;
    if raw.capture_finalized && start == Some(0) && dropped == Some(0) {
        result.reasons[7] |= end.is_some_and(|n| n != result.pending_requests);
    }
    Ok(result)
}
