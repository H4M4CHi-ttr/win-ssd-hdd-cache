//! Loss-preserving native framing and independently derived history checks.
use crate::wire::{HEADER_BYTES, decode_frame, get32, get64, put32, put64, zero};
use crate::{
    Command, Frame, MAX_ARCHIVE_BYTES, MAX_FRAMES, MAX_RECORDS, Observation, ProbeError, Request,
    decode_response,
};
use cache_capture_core::{Operation, Submission};
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    num::NonZeroU64,
};

const PREFIX_BYTES: usize = 64;
const FOOTER_BYTES: usize = 64;
const HISTORY_FAULT: u32 = 16;
const REASONS: [&str; 15] = [
    "EMPTY_CAPTURE",
    "MISSING_TERMINAL",
    "COLLECTOR_FAULT",
    "PRODUCER_FAULT",
    "UNKNOWN_COUNTERS",
    "DROPPED_EVENTS",
    "SKIPPED_OBSERVATIONS",
    "CALL_SEQUENCE_GAP",
    "SEQUENCE_ORDER",
    "SEQUENCE_GAP",
    "COUNT_MISMATCH",
    "DUPLICATE_REQUEST",
    "UNPAIRED_REQUEST",
    "UNVERIFIED_BOUNDARIES",
    "UNVERIFIED_PROVENANCE",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CollectorFault {
    TransportError = 1,
    BadResponse = 2,
    LimitReached = 4,
    TerminalTimeout = 8,
    InconsistentHistory = 16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Footer {
    frames: u32,
    records: u32,
    faults: u32,
    terminal_index: u32,
    terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    pub(crate) session: u64,
    pub(crate) frames: Vec<Frame>,
    footer: Option<Footer>,
    records: usize,
}

impl Archive {
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }
    pub const fn record_count(&self) -> usize {
        self.records
    }
    pub(crate) fn audit(&self) -> NativeAudit {
        analyze(self)
    }
    pub(crate) fn terminal(&self) -> Option<&Frame> {
        if valid_terminal(self) {
            self.frames.last()
        } else {
            None
        }
    }
}

pub fn decode_archive(bytes: &[u8]) -> Result<Archive, ProbeError> {
    use ProbeError::{InvalidArchive, TooLarge};
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(TooLarge);
    }
    if bytes.len() < PREFIX_BYTES
        || &bytes[..8] != b"WCHP04AR"
        || get32(bytes, 8)? != 1
        || get32(bytes, 12)? != 64
        || get32(bytes, 16)? != 1
        || get32(bytes, 20)? != 144
        || get32(bytes, 24)? != 96
        || get32(bytes, 28)? != 0
        || get64(bytes, 32)? == 0
        || get64(bytes, 40)? != 1
        || get32(bytes, 48)? != 4096
        || get32(bytes, 52)? != 4096
        || !zero(bytes, 56..64)
    {
        return Err(InvalidArchive);
    }
    let session = get64(bytes, 32)?;
    let mut at = PREFIX_BYTES;
    let mut frames = Vec::new();
    let mut records = 0usize;
    let mut footer = None;
    while at < bytes.len() {
        let rest = &bytes[at..];
        if rest.starts_with(b"WCHP04EN") {
            if rest.len() != FOOTER_BYTES
                || get32(rest, 8)? != 1
                || get32(rest, 12)? != 64
                || get32(rest, 24)? & !0x1f != 0
                || get64(rest, 32)? != session
                || get64(rest, 40)? != 1
                || get32(rest, 48)? > 1
                || !zero(rest, 52..64)
            {
                return Err(InvalidArchive);
            }
            footer = Some(Footer {
                frames: get32(rest, 16)?,
                records: get32(rest, 20)?,
                faults: get32(rest, 24)?,
                terminal_index: get32(rest, 28)?,
                terminal: get32(rest, 48)? != 0,
            });
            at = bytes.len();
        } else {
            // Validate the entire candidate frame before testing frame/record caps.
            // A partial 4097th frame is malformed, not a complete over-cap frame.
            if rest.len() < HEADER_BYTES {
                return Err(InvalidArchive);
            }
            let length = get32(rest, 16)? as usize;
            let frame = decode_frame(rest.get(..length).ok_or(InvalidArchive)?)?;
            if frame.session != session {
                return Err(InvalidArchive);
            }
            records = records.checked_add(frame.records.len()).ok_or(TooLarge)?;
            if frames.len() >= MAX_FRAMES || records > MAX_RECORDS {
                return Err(TooLarge);
            }
            frames.push(frame);
            at = at.checked_add(length).ok_or(InvalidArchive)?;
        }
    }
    Ok(Archive {
        session,
        frames,
        footer,
        records,
    })
}

pub fn audit_archive(bytes: &[u8]) -> Result<NativeAudit, ProbeError> {
    Ok(decode_archive(bytes)?.audit())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeAudit {
    frames: usize,
    records: usize,
    terminal: bool,
    reconciled: bool,
    reasons: [bool; 15],
    derived_faults: u32,
}

impl NativeAudit {
    pub const fn retained_records_reconciled(&self) -> bool {
        self.reconciled
    }
    pub const fn terminal_received(&self) -> bool {
        self.terminal
    }
    pub fn to_json(&self) -> String {
        let reasons = REASONS
            .iter()
            .zip(self.reasons)
            .filter(|(_, set)| *set)
            .map(|(text, _)| format!("\"{text}\""))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema_version\":1,\"format\":\"p04-native-audit\",\"verdict\":\"unqualified\",\"frame_count\":\"{}\",\"record_count\":\"{}\",\"terminal_received\":{},\"retained_records_reconciled\":{},\"reasons\":[{}],\"producer_verified\":false,\"authorization_verified\":false,\"coverage_verified\":false,\"kernel_runtime_qualified\":false}}\n",
            self.frames, self.records, self.terminal, self.reconciled, reasons
        )
    }
}

fn valid_terminal(archive: &Archive) -> bool {
    let Some(footer) = &archive.footer else {
        return false;
    };
    footer.terminal
        && archive.frames.len().checked_sub(1) == Some(footer.terminal_index as usize)
        && footer.frames as usize == archive.frames.len()
        && footer.records as usize == archive.records
        && terminal_candidate(archive)
}

fn terminal_candidate(archive: &Archive) -> bool {
    archive
        .frames
        .last()
        .is_some_and(|last| last.state == 3 && last.active == 0 && last.queued == 0)
        && archive
            .frames
            .iter()
            .any(|frame| frame.command == Command::Stop && matches!(frame.state, 2 | 3))
}

fn first_baseline(frame: &Frame) -> bool {
    frame.command == Command::Begin
        && frame.call == 1
        && frame.state == 1
        && frame.pnp == 1
        && frame.active == 0
        && frame.queued == 0
        && frame.known == 7
        && frame.faults == 0
        && frame.attempted == Some(0)
        && frame.dropped == Some(0)
        && frame.popped == Some(0)
        && frame.skipped == Some(0)
}

fn analyze(archive: &Archive) -> NativeAudit {
    let mut reasons = [false; 15];
    reasons[0] = archive.records == 0;
    let terminal = valid_terminal(archive);
    reasons[1] = !terminal;
    reasons[4] = !terminal;
    reasons[13] = true;
    reasons[14] = true;
    let mut inconsistent = false;
    let mut previous: Option<&Frame> = None;
    let mut previous_seq: Option<u64> = None;
    let mut submits: BTreeMap<u64, Submission> = BTreeMap::new();
    let mut completions = BTreeSet::new();
    let mut pending = BTreeSet::new();
    let mut orphan = false;
    for frame in &archive.frames {
        reasons[3] |= frame.faults != 0;
        reasons[4] |= frame.known != 7;
        reasons[5] |= frame.dropped.is_some_and(|n| n > 0);
        reasons[6] |= frame.skipped.is_some_and(|n| n > 0);
        if let Some(prev) = previous {
            reasons[7] |= prev.call.checked_add(1) != Some(frame.call);
            inconsistent |= frame.command == Command::Begin
                || frame.state < prev.state
                || frame.known & !prev.known != 0
                || frame.faults & prev.faults != prev.faults;
            for (bit, exhaustion) in [(1, 0x004), (2, 0x080), (4, 0x040)] {
                inconsistent |= prev.known & bit != 0
                    && frame.known & bit == 0
                    && frame.faults & exhaustion == 0;
            }
            for (old, new) in [
                (prev.attempted, frame.attempted),
                (prev.dropped, frame.dropped),
                (prev.popped, frame.popped),
                (prev.skipped, frame.skipped),
            ] {
                if let (Some(old), Some(new)) = (old, new) {
                    inconsistent |= new < old;
                }
            }
            if let (Some(old), Some(new)) = (prev.popped, frame.popped) {
                reasons[10] |= old.checked_add(frame.records.len() as u64) != Some(new);
            }
        } else {
            inconsistent |= !first_baseline(frame);
            reasons[7] |= frame.call != 1;
        }
        inconsistent |= frame.command == Command::Stop && frame.state == 1;
        inconsistent |= frame.state == 3 && frame.active != 0;
        if let (Some(attempted), Some(dropped), Some(popped)) =
            (frame.attempted, frame.dropped, frame.popped)
        {
            reasons[10] |= (frame.queued as u64)
                .checked_add(popped)
                .and_then(|v| v.checked_add(dropped))
                != Some(attempted);
        }
        for record in &frame.records {
            if let Some(prev) = previous_seq {
                reasons[8] |= record.seq <= prev;
                reasons[9] |= prev.checked_add(1) != Some(record.seq);
            } else {
                reasons[9] |= record.seq != 0;
            }
            previous_seq = Some(record.seq);
            match record.observation {
                Observation::Submit(submission) => match submits.entry(record.request_id) {
                    Entry::Vacant(entry) => {
                        entry.insert(submission);
                        pending.insert(record.request_id);
                    }
                    Entry::Occupied(_) => reasons[11] = true,
                },
                Observation::Complete(completion) => {
                    // Classification uncertainty is retained even on a duplicate.
                    if let Some(saved) = submits.get(&record.request_id) {
                        inconsistent |= completion.information.is_some()
                            && !matches!(saved.operation(), Operation::Read | Operation::Write);
                    }
                    if !completions.insert(record.request_id) {
                        reasons[11] = true;
                    } else if !pending.remove(&record.request_id) {
                        orphan = true;
                    }
                }
            }
        }
        previous = Some(frame);
    }
    reasons[12] = orphan || !pending.is_empty();
    if let Some(footer) = &archive.footer {
        reasons[10] |= footer.frames as usize != archive.frames.len()
            || footer.records as usize != archive.records;
        inconsistent |= footer.terminal && !terminal;
        inconsistent |= !footer.terminal && footer.terminal_index != u32::MAX;
        inconsistent |= !footer.terminal && terminal_candidate(archive);
    }
    if let Some(last) = archive.terminal() {
        if let Some(attempted) = last.attempted {
            reasons[9] |= match previous_seq {
                Some(seq) => seq.checked_add(1) != Some(attempted),
                None => attempted != 0,
            };
        }
        if let (Some(attempted), Some(dropped)) = (last.attempted, last.dropped) {
            reasons[10] |= (archive.records as u64).checked_add(dropped) != Some(attempted);
        }
        if let Some(popped) = last.popped {
            reasons[10] |= popped != archive.records as u64;
        }
    }
    inconsistent |= reasons[7] || reasons[8] || reasons[10] || reasons[11];
    let derived_faults = if inconsistent { HISTORY_FAULT } else { 0 };
    reasons[2] = derived_faults | archive.footer.as_ref().map_or(0, |f| f.faults) != 0;
    let reconciled = !reasons[..13].iter().any(|r| *r);
    NativeAudit {
        frames: archive.frames.len(),
        records: archive.records,
        terminal,
        reconciled,
        reasons,
        derived_faults,
    }
}

fn prefix(session: u64) -> [u8; PREFIX_BYTES] {
    let mut bytes = [0; PREFIX_BYTES];
    bytes[..8].copy_from_slice(b"WCHP04AR");
    for (at, value) in [
        (8, 1),
        (12, 64),
        (16, 1),
        (20, 144),
        (24, 96),
        (48, 4096),
        (52, 4096),
    ] {
        put32(&mut bytes, at, value);
    }
    put64(&mut bytes, 32, session);
    put64(&mut bytes, 40, 1);
    bytes
}

/// Only one command may be outstanding. Stored bytes are exactly the responses
/// that passed binding/static validation; uncertain exchanges never get retried.
#[derive(Debug)]
pub struct CaptureBuilder {
    session: NonZeroU64,
    next_call: Option<NonZeroU64>,
    outstanding: Option<Request>,
    begin_attempted: bool,
    begun: bool,
    stop_acknowledged: bool,
    uncertain: bool,
    cleanup_stop_attempted: bool,
    faults: u32,
    frames: Vec<Frame>,
    raw_frames: Vec<Vec<u8>>,
    records: usize,
}

impl CaptureBuilder {
    pub fn new(session: NonZeroU64) -> Self {
        Self {
            session,
            next_call: NonZeroU64::new(1),
            outstanding: None,
            begin_attempted: false,
            begun: false,
            stop_acknowledged: false,
            uncertain: false,
            cleanup_stop_attempted: false,
            faults: 0,
            frames: Vec::new(),
            raw_frames: Vec::new(),
            records: 0,
        }
    }
    pub fn next_request(&mut self, command: Command, maximum: u32) -> Result<Request, ProbeError> {
        use ProbeError::{InvalidArchive, TooLarge};
        if self.outstanding.is_some() || self.terminal_received() {
            return Err(InvalidArchive);
        }
        if (!self.begun && (command != Command::Begin || self.begin_attempted))
            || (self.begun && command == Command::Begin)
            || (self.uncertain && (command != Command::Stop || self.cleanup_stop_attempted))
        {
            return Err(InvalidArchive);
        }
        if self.remaining_frames() == 0
            || (command == Command::Drain && maximum as usize > self.remaining_records())
        {
            return Err(TooLarge);
        }
        if command == Command::Drain && !self.stop_acknowledged && self.remaining_frames() <= 2 {
            return Err(TooLarge);
        }
        let call = self.next_call.ok_or(TooLarge)?;
        let request = Request::new(command, self.session, call, maximum)?;
        self.next_call = call.get().checked_add(1).and_then(NonZeroU64::new);
        self.begin_attempted |= command == Command::Begin;
        if self.uncertain {
            self.cleanup_stop_attempted = true;
        }
        self.outstanding = Some(request);
        Ok(request)
    }

    pub fn receive(&mut self, request: Request, bytes: &[u8]) -> Result<(), ProbeError> {
        if self.outstanding != Some(request) {
            return Err(ProbeError::InvalidArchive);
        }
        let frame = match decode_response(request, bytes) {
            Ok(frame) => frame,
            Err(error) => {
                self.fail(request, CollectorFault::BadResponse)?;
                return Err(error);
            }
        };
        if self.remaining_frames() == 0 || frame.records.len() > self.remaining_records() {
            self.fail(request, CollectorFault::LimitReached)?;
            return Err(ProbeError::TooLarge);
        }
        self.outstanding = None;
        self.begun |= frame.command == Command::Begin;
        self.stop_acknowledged |= frame.command == Command::Stop && matches!(frame.state, 2 | 3);
        self.records += frame.records.len();
        self.frames.push(frame);
        self.raw_frames.push(bytes.to_vec());
        let interim = Archive {
            session: self.session.get(),
            frames: self.frames.clone(),
            footer: None,
            records: self.records,
        };
        if interim.audit().derived_faults != 0 {
            self.faults |= HISTORY_FAULT;
            self.uncertain = true;
        }
        Ok(())
    }

    pub fn fail(&mut self, request: Request, fault: CollectorFault) -> Result<(), ProbeError> {
        if self.outstanding != Some(request) {
            return Err(ProbeError::InvalidArchive);
        }
        self.outstanding = None;
        self.faults |= fault as u32;
        self.uncertain = true;
        Ok(())
    }

    /// Records a limit/deadline without fabricating a failed exchange or event.
    pub fn latch_fault(&mut self, fault: CollectorFault) {
        self.faults |= fault as u32;
    }
    pub fn remaining_records(&self) -> usize {
        MAX_RECORDS - self.records
    }
    pub fn remaining_frames(&self) -> usize {
        MAX_FRAMES - self.frames.len()
    }
    pub const fn normal_draining_allowed(&self) -> bool {
        self.begun && !self.uncertain
    }
    pub fn last_frame(&self) -> Option<&Frame> {
        self.frames.last()
    }
    pub fn terminal_received(&self) -> bool {
        self.stop_acknowledged
            && self
                .frames
                .last()
                .is_some_and(|f| f.state == 3 && f.active == 0 && f.queued == 0)
    }

    pub fn finish(self) -> Result<Vec<u8>, ProbeError> {
        if !self.begun || self.outstanding.is_some() {
            return Err(ProbeError::InvalidArchive);
        }
        let terminal = self.terminal_received();
        let mut bytes = prefix(self.session.get()).to_vec();
        for frame in &self.raw_frames {
            bytes.extend_from_slice(frame);
        }
        let mut footer = [0; FOOTER_BYTES];
        footer[..8].copy_from_slice(b"WCHP04EN");
        put32(&mut footer, 8, 1);
        put32(&mut footer, 12, 64);
        put32(&mut footer, 16, self.frames.len() as u32);
        put32(&mut footer, 20, self.records as u32);
        put32(&mut footer, 24, self.faults);
        put32(
            &mut footer,
            28,
            if terminal {
                (self.frames.len() - 1) as u32
            } else {
                u32::MAX
            },
        );
        put64(&mut footer, 32, self.session.get());
        put64(&mut footer, 40, 1);
        put32(&mut footer, 48, u32::from(terminal));
        bytes.extend_from_slice(&footer);
        // Defense in depth: emitter never returns a static-invalid archive.
        decode_archive(&bytes)?;
        Ok(bytes)
    }
}
