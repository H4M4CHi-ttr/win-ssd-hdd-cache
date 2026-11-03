//! Conservative P03 v2 projection; no boundary or target-address evidence is inferred.
use crate::{Observation, ProbeError, decode_archive};
use cache_capture_core::Operation;
use cache_types::capture::{CAPTURE_MAX_BYTES, audit_capture};
use serde::Serialize;

#[derive(Serialize)]
struct Envelope {
    schema_version: u32,
    evidence_kind: &'static str,
    observation_point: &'static str,
    session_id: String,
    windows_version: Option<&'static str>,
    source_revision: Option<&'static str>,
    producer_sha256: Option<&'static str>,
    capture_finalized: bool,
    start_inflight: Option<String>,
    end_inflight: Option<String>,
    events_attempted: Option<String>,
    events_dropped: Option<String>,
    records: Vec<ProjectedRecord>,
}

#[derive(Serialize)]
struct ProjectedRecord {
    seq: String,
    phase: &'static str,
    request_id: String,
    target_id: &'static str,
    operation: Option<&'static str>,
    offset: Option<String>,
    length: Option<String>,
    status: Option<String>,
    information: Option<String>,
}

pub fn project_v2(bytes: &[u8]) -> Result<String, ProbeError> {
    let archive = decode_archive(bytes)?;
    let report = archive.audit();
    let terminal = archive.terminal();
    let records = archive
        .frames
        .iter()
        .flat_map(|frame| &frame.records)
        .map(|record| {
            let (phase, operation, status, information) = match record.observation {
                Observation::Submit(s) => (
                    "submit",
                    Some(match s.operation() {
                        Operation::Read => "read",
                        Operation::Write => "write",
                        Operation::Flush => "flush",
                        Operation::Other => "other",
                    }),
                    None,
                    None,
                ),
                Observation::Complete(c) => (
                    "complete",
                    None,
                    Some(c.status.to_string()),
                    c.information.map(|v| v.to_string()),
                ),
            };
            ProjectedRecord {
                seq: record.seq.to_string(),
                phase,
                request_id: record.request_id.to_string(),
                target_id: "1",
                operation,
                offset: None,
                length: None,
                status,
                information,
            }
        })
        .collect();
    let envelope = Envelope {
        schema_version: 2,
        evidence_kind: "windows_observation",
        observation_point: "disk_class_upper",
        session_id: archive.session.to_string(),
        windows_version: None,
        source_revision: None,
        producer_sha256: None,
        capture_finalized: report.retained_records_reconciled(),
        start_inflight: None,
        end_inflight: None,
        events_attempted: terminal.and_then(|f| f.attempted).map(|n| n.to_string()),
        events_dropped: terminal.and_then(|f| f.dropped).map(|n| n.to_string()),
        records,
    };
    let mut output = serde_json::to_string(&envelope).map_err(|_| ProbeError::InvalidArchive)?;
    output.push('\n');
    if output.len() > CAPTURE_MAX_BYTES {
        return Err(ProbeError::ProjectionTooLarge);
    }
    // The accepted P03 parser is the final projection boundary, not an assertion
    // of capture authenticity or Windows coverage.
    audit_capture(&output).map_err(|_| ProbeError::InvalidArchive)?;
    Ok(output)
}
