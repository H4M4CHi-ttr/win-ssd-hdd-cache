//! Bounded trace-v1 input, not a scheduler, cache engine, or safety oracle.
use crate::{
    range::checked_range,
    wire::{NullableString, Object, UnsignedToken, decimal_u64},
};
use serde::{
    Deserialize, Deserializer,
    de::{self, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt, marker::PhantomData};

pub const TRACE_MAX_BYTES: usize = 67_108_864;
pub const TRACE_MAX_CAPACITY: usize = 1_048_576;
pub const TRACE_MAX_EVENTS: usize = 10_000;
pub const TRACE_MAX_PAYLOAD_BYTES: usize = 16_777_216;
const EVENT_LIMIT_MARKER: &str = "TRACE_V1_EVENT_LIMIT";

/// Only `parse_trace` constructs this validated container; getters cannot mutate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceV1 {
    seed: u64,
    capacity: u64,
    line_size: u32,
    slot_count: u16,
    counter_limit: u64,
    initial: Vec<u8>,
    events: Vec<TraceEvent>,
}

impl TraceV1 {
    pub const fn seed(&self) -> u64 {
        self.seed
    }
    pub const fn capacity(&self) -> u64 {
        self.capacity
    }
    pub const fn line_size(&self) -> u32 {
        self.line_size
    }
    pub const fn slot_count(&self) -> u16 {
        self.slot_count
    }
    pub const fn counter_limit(&self) -> u64 {
        self.counter_limit
    }
    pub fn initial(&self) -> &[u8] {
        &self.initial
    }
    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoTarget {
    OriginRead,
    OriginWrite,
    OriginDiscard,
    OriginFlush,
    CacheRead,
    CacheWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoStatus {
    Ok,
    IoError,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidateReason {
    Disable,
    Suspend,
    TargetChange,
    Fault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Accepted,
    Rejected,
    Pending,
}

/// Typed syntax. A well-formed event may still be rejected/pending or a runtime
/// semantic violation. Do not infer acceptance or physical completion from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEvent {
    ReadSubmit {
        request: u64,
        offset: u64,
        length: u64,
    },
    WriteSubmit {
        request: u64,
        offset: u64,
        data: Vec<u8>,
        fua: bool,
    },
    DiscardSubmit {
        request: u64,
        offset: u64,
        length: u64,
    },
    FlushSubmit {
        request: u64,
    },
    FillBegin {
        request: u64,
        line: u64,
    },
    IoSubmit {
        io: u64,
        request: u64,
        target: IoTarget,
        slot: Option<u64>,
        source: Option<u64>,
    },
    IoTransfer {
        io: u64,
        offset: u64,
        length: u64,
    },
    IoComplete {
        io: u64,
        status: IoStatus,
        reported: u64,
    },
    CancelRequest {
        request: u64,
    },
    CancelIo {
        io: u64,
    },
    RetireRequest {
        request: u64,
    },
    CachePublish {
        io: u64,
    },
    ReadCommit {
        request: u64,
        io: u64,
    },
    MutationCommit {
        request: u64,
        io: u64,
    },
    EpochInvalidate {
        reason: InvalidateReason,
    },
    SlotBind {
        slot: u64,
        line: u64,
    },
    ResumeCache,
    OriginPersist {
        io: u64,
        offset: u64,
        length: u64,
    },
    PowerLoss,
    AssertRead {
        request: u64,
        data: Vec<u8>,
        known: Vec<bool>,
    },
    AssertResult {
        request: u64,
        status: IoStatus,
        reported: u64,
    },
    AssertCache {
        slot: u64,
        valid: bool,
    },
    AssertDecision {
        index: u64,
        decision: Decision,
    },
    AssertOrigin {
        offset: u64,
        data: Vec<u8>,
        known: Vec<bool>,
    },
}

impl TraceEvent {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ReadSubmit { .. } => "read_submit",
            Self::WriteSubmit { .. } => "write_submit",
            Self::DiscardSubmit { .. } => "discard_submit",
            Self::FlushSubmit { .. } => "flush_submit",
            Self::FillBegin { .. } => "fill_begin",
            Self::IoSubmit { .. } => "io_submit",
            Self::IoTransfer { .. } => "io_transfer",
            Self::IoComplete { .. } => "io_complete",
            Self::CancelRequest { .. } => "cancel_request",
            Self::CancelIo { .. } => "cancel_io",
            Self::RetireRequest { .. } => "retire_request",
            Self::CachePublish { .. } => "cache_publish",
            Self::ReadCommit { .. } => "read_commit",
            Self::MutationCommit { .. } => "mutation_commit",
            Self::EpochInvalidate { .. } => "epoch_invalidate",
            Self::SlotBind { .. } => "slot_bind",
            Self::ResumeCache => "resume_cache",
            Self::OriginPersist { .. } => "origin_persist",
            Self::PowerLoss => "power_loss",
            Self::AssertRead { .. } => "assert_read",
            Self::AssertResult { .. } => "assert_result",
            Self::AssertCache { .. } => "assert_cache",
            Self::AssertDecision { .. } => "assert_decision",
            Self::AssertOrigin { .. } => "assert_origin",
        }
    }
    pub const fn is_assert(&self) -> bool {
        matches!(
            self,
            Self::AssertRead { .. }
                | Self::AssertResult { .. }
                | Self::AssertCache { .. }
                | Self::AssertDecision { .. }
                | Self::AssertOrigin { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceErrorCode {
    InputTooLarge,
    InvalidJson,
    InvalidSchema,
    InvalidValue,
    LimitExceeded,
    InvalidReference,
}

impl TraceErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::InvalidJson => "INVALID_JSON",
            Self::InvalidSchema => "INVALID_SCHEMA",
            Self::InvalidValue => "INVALID_VALUE",
            Self::LimitExceeded => "LIMIT_EXCEEDED",
            Self::InvalidReference => "INVALID_REFERENCE",
        }
    }
}

/// Contains no input payload, serde message, path or caller-supplied text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceError {
    kind: TraceErrorCode,
    event_index: Option<usize>,
}

impl TraceError {
    pub const fn code(&self) -> &'static str {
        self.kind.as_str()
    }
    pub const fn event_index(&self) -> Option<usize> {
        self.event_index
    }
    fn new(kind: TraceErrorCode, event_index: Option<usize>) -> Self {
        Self { kind, event_index }
    }
}
impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())?;
        if let Some(index) = self.event_index {
            write!(f, " at event {index}")?;
        }
        Ok(())
    }
}
impl std::error::Error for TraceError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvelope<'a> {
    schema_version: UnsignedToken,
    seed: String,
    capacity: String,
    line_size: UnsignedToken,
    slot_count: UnsignedToken,
    counter_limit: String,
    initial_hex: String,
    #[serde(borrow)]
    events: &'a RawValue,
}

struct EventSlices<'a>(Vec<&'a RawValue>);
impl<'de: 'a, 'a> Deserialize<'de> for EventSlices<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EventsVisitor<'a>(PhantomData<&'a ()>);
        impl<'de: 'a, 'a> Visitor<'de> for EventsVisitor<'a> {
            type Value = EventSlices<'a>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded event array")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
                // Borrow the raw JSON; do not allocate a giant Vec<Event> and only
                // then test its length. The one excess reference is never retained.
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<&RawValue>()? {
                    if items.len() == TRACE_MAX_EVENTS {
                        return Err(de::Error::custom(EVENT_LIMIT_MARKER));
                    }
                    items.push(item);
                }
                Ok(EventSlices(items))
            }
        }
        deserializer.deserialize_seq(EventsVisitor(PhantomData))
    }
}

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
enum RawEvent {
    ReadSubmit {
        request: String,
        offset: String,
        length: String,
    },
    WriteSubmit {
        request: String,
        offset: String,
        data: String,
        fua: bool,
    },
    DiscardSubmit {
        request: String,
        offset: String,
        length: String,
    },
    FlushSubmit {
        request: String,
    },
    FillBegin {
        request: String,
        line: String,
    },
    IoSubmit {
        io: String,
        request: String,
        #[serde(deserialize_with = "enum_string")]
        target: IoTarget,
        slot: NullableString,
        source: NullableString,
    },
    IoTransfer {
        io: String,
        offset: String,
        length: String,
    },
    IoComplete {
        io: String,
        #[serde(deserialize_with = "enum_string")]
        status: IoStatus,
        reported: String,
    },
    CancelRequest {
        request: String,
    },
    CancelIo {
        io: String,
    },
    RetireRequest {
        request: String,
    },
    CachePublish {
        io: String,
    },
    ReadCommit {
        request: String,
        io: String,
    },
    MutationCommit {
        request: String,
        io: String,
    },
    EpochInvalidate {
        #[serde(deserialize_with = "enum_string")]
        reason: InvalidateReason,
    },
    SlotBind {
        slot: String,
        line: String,
    },
    ResumeCache {},
    OriginPersist {
        io: String,
        offset: String,
        length: String,
    },
    PowerLoss {},
    AssertRead {
        request: String,
        data: String,
        known: String,
    },
    AssertResult {
        request: String,
        #[serde(deserialize_with = "enum_string")]
        status: IoStatus,
        reported: String,
    },
    AssertCache {
        slot: String,
        valid: bool,
    },
    AssertDecision {
        index: String,
        #[serde(deserialize_with = "enum_string")]
        decision: Decision,
    },
    AssertOrigin {
        offset: String,
        data: String,
        known: String,
    },
}

// Internally tagged enum buffering can otherwise let serde's enum visitor accept
// an externally tagged map such as {"ok":null}. The wire requires a string.
fn enum_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value = String::deserialize(deserializer)?;
    T::deserialize(de::value::StringDeserializer::<D::Error>::new(value))
}

fn json_error(error: serde_json::Error, index: Option<usize>) -> TraceError {
    // Only our exact leading sentinel (not substring matching user keys) maps to
    // the event limit. Serde messages are dropped and never returned to a caller.
    let kind = if error.is_data()
        && error
            .to_string()
            .starts_with(&format!("{EVENT_LIMIT_MARKER} at line "))
    {
        TraceErrorCode::LimitExceeded
    } else if error.is_data() {
        TraceErrorCode::InvalidSchema
    } else {
        TraceErrorCode::InvalidJson
    };
    TraceError::new(kind, index)
}

type Checked<T> = Result<T, TraceErrorCode>;
fn number(value: &str) -> Checked<u64> {
    decimal_u64(value).ok_or(TraceErrorCode::InvalidValue)
}
fn identifier(value: &str) -> Checked<u64> {
    let value = number(value)?;
    if value == 0 {
        Err(TraceErrorCode::InvalidValue)
    } else {
        Ok(value)
    }
}
fn optional(value: NullableString, positive: bool) -> Checked<Option<u64>> {
    value
        .0
        .map(|v| if positive { identifier(&v) } else { number(&v) })
        .transpose()
}

struct PayloadBudget {
    used: usize,
    capacity: usize,
}
fn hex_length(hex: &str) -> Checked<usize> {
    let bytes = hex.as_bytes();
    if !bytes.len().is_multiple_of(2)
        || !bytes
            .iter()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err(TraceErrorCode::InvalidValue);
    }
    Ok(bytes.len() / 2)
}
impl PayloadBudget {
    fn decode(&mut self, hex: &str) -> Checked<Vec<u8>> {
        let len = hex_length(hex)?;
        if len > self.capacity {
            return Err(TraceErrorCode::InvalidValue);
        }
        let next = self
            .used
            .checked_add(len)
            .filter(|n| *n <= TRACE_MAX_PAYLOAD_BYTES)
            .ok_or(TraceErrorCode::LimitExceeded)?;
        // Reserve the decoded budget before allocation or copying the payload.
        self.used = next;
        let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        Ok(hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| digit(p[0]) * 16 + digit(p[1]))
            .collect())
    }
    fn masked(&mut self, hex: &str, known: &str) -> Checked<(Vec<u8>, Vec<bool>)> {
        // Mask bounds and unknown placeholders are checked before mask allocation.
        if known.len() != hex.len() / 2 || !known.bytes().all(|b| b == b'0' || b == b'1') {
            return Err(TraceErrorCode::InvalidValue);
        }
        let data = self.decode(hex)?;
        if data
            .iter()
            .zip(known.bytes())
            .any(|(v, k)| k == b'0' && *v != 0)
        {
            return Err(TraceErrorCode::InvalidValue);
        }
        Ok((data, known.bytes().map(|b| b == b'1').collect()))
    }
}

fn convert(raw: RawEvent, budget: &mut PayloadBudget) -> Checked<TraceEvent> {
    use TraceEvent as E;
    Ok(match raw {
        RawEvent::ReadSubmit {
            request,
            offset,
            length,
        } => E::ReadSubmit {
            request: identifier(&request)?,
            offset: number(&offset)?,
            length: number(&length)?,
        },
        RawEvent::WriteSubmit {
            request,
            offset,
            data,
            fua,
        } => E::WriteSubmit {
            request: identifier(&request)?,
            offset: number(&offset)?,
            data: budget.decode(&data)?,
            fua,
        },
        RawEvent::DiscardSubmit {
            request,
            offset,
            length,
        } => E::DiscardSubmit {
            request: identifier(&request)?,
            offset: number(&offset)?,
            length: number(&length)?,
        },
        RawEvent::FlushSubmit { request } => E::FlushSubmit {
            request: identifier(&request)?,
        },
        RawEvent::FillBegin { request, line } => E::FillBegin {
            request: identifier(&request)?,
            line: number(&line)?,
        },
        RawEvent::IoSubmit {
            io,
            request,
            target,
            slot,
            source,
        } => E::IoSubmit {
            io: identifier(&io)?,
            request: identifier(&request)?,
            target,
            slot: optional(slot, false)?,
            source: optional(source, true)?,
        },
        RawEvent::IoTransfer { io, offset, length } => E::IoTransfer {
            io: identifier(&io)?,
            offset: number(&offset)?,
            length: number(&length)?,
        },
        RawEvent::IoComplete {
            io,
            status,
            reported,
        } => E::IoComplete {
            io: identifier(&io)?,
            status,
            reported: number(&reported)?,
        },
        RawEvent::CancelRequest { request } => E::CancelRequest {
            request: identifier(&request)?,
        },
        RawEvent::CancelIo { io } => E::CancelIo {
            io: identifier(&io)?,
        },
        RawEvent::RetireRequest { request } => E::RetireRequest {
            request: identifier(&request)?,
        },
        RawEvent::CachePublish { io } => E::CachePublish {
            io: identifier(&io)?,
        },
        RawEvent::ReadCommit { request, io } => E::ReadCommit {
            request: identifier(&request)?,
            io: identifier(&io)?,
        },
        RawEvent::MutationCommit { request, io } => E::MutationCommit {
            request: identifier(&request)?,
            io: identifier(&io)?,
        },
        RawEvent::EpochInvalidate { reason } => E::EpochInvalidate { reason },
        RawEvent::SlotBind { slot, line } => E::SlotBind {
            slot: number(&slot)?,
            line: number(&line)?,
        },
        RawEvent::ResumeCache {} => E::ResumeCache,
        RawEvent::OriginPersist { io, offset, length } => E::OriginPersist {
            io: identifier(&io)?,
            offset: number(&offset)?,
            length: number(&length)?,
        },
        RawEvent::PowerLoss {} => E::PowerLoss,
        RawEvent::AssertRead {
            request,
            data,
            known,
        } => {
            let (data, known) = budget.masked(&data, &known)?;
            E::AssertRead {
                request: identifier(&request)?,
                data,
                known,
            }
        }
        RawEvent::AssertResult {
            request,
            status,
            reported,
        } => E::AssertResult {
            request: identifier(&request)?,
            status,
            reported: number(&reported)?,
        },
        RawEvent::AssertCache { slot, valid } => E::AssertCache {
            slot: number(&slot)?,
            valid,
        },
        RawEvent::AssertDecision { index, decision } => E::AssertDecision {
            index: number(&index)?,
            decision,
        },
        RawEvent::AssertOrigin {
            offset,
            data,
            known,
        } => {
            let (data, known) = budget.masked(&data, &known)?;
            E::AssertOrigin {
                offset: number(&offset)?,
                data,
                known,
            }
        }
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParentKind {
    Read,
    Write,
    Discard,
    Flush,
    Fill,
}
#[derive(Clone, Copy)]
struct Parent {
    kind: ParentKind,
    length: u64,
}
#[derive(Clone, Copy)]
struct Child {
    request: u64,
    target: IoTarget,
    length: u64,
}

struct StaticReferences {
    parents: BTreeMap<u64, Parent>,
    children: BTreeMap<u64, Child>,
    capacity: u64,
    line_size: u64,
    slot_count: u64,
}
impl StaticReferences {
    // Payload-dependent static ranges and references precede decoded allocation.
    // This does not execute phases, claims, coverage, or completion decisions.
    fn preflight_payload(&self, event: &RawEvent) -> Checked<()> {
        match event {
            RawEvent::WriteSubmit {
                request,
                offset,
                data,
                ..
            } => {
                let id = identifier(request)?;
                if self.parents.contains_key(&id) {
                    return Err(TraceErrorCode::InvalidReference);
                }
                self.range(number(offset)?, hex_length(data)? as u64, self.capacity)
            }
            RawEvent::AssertOrigin { offset, data, .. } => {
                self.range(number(offset)?, hex_length(data)? as u64, self.capacity)
            }
            RawEvent::AssertRead { request, data, .. } => {
                let parent = self.parent(identifier(request)?)?;
                if parent.kind != ParentKind::Read {
                    return Err(TraceErrorCode::InvalidReference);
                }
                self.range(0, hex_length(data)? as u64, parent.length)
            }
            _ => Ok(()),
        }
    }

    fn range(&self, offset: u64, length: u64, capacity: u64) -> Checked<()> {
        checked_range(offset, length, capacity)
            .map(|_| ())
            .map_err(|_| TraceErrorCode::InvalidValue)
    }
    fn line_length(&self, line: u64) -> Checked<u64> {
        let start = line
            .checked_mul(self.line_size)
            .filter(|s| *s < self.capacity)
            .ok_or(TraceErrorCode::InvalidValue)?;
        Ok((self.capacity - start).min(self.line_size))
    }
    fn slot(&self, slot: u64) -> Checked<()> {
        if slot < self.slot_count {
            Ok(())
        } else {
            Err(TraceErrorCode::InvalidValue)
        }
    }
    fn parent(&self, id: u64) -> Checked<Parent> {
        self.parents
            .get(&id)
            .copied()
            .ok_or(TraceErrorCode::InvalidReference)
    }
    fn child(&self, id: u64) -> Checked<Child> {
        self.children
            .get(&id)
            .copied()
            .ok_or(TraceErrorCode::InvalidReference)
    }
    fn declare(&mut self, request: u64, kind: ParentKind, offset: u64, length: u64) -> Checked<()> {
        self.range(offset, length, self.capacity)?;
        if self.parents.contains_key(&request) {
            return Err(TraceErrorCode::InvalidReference);
        }
        self.parents.insert(request, Parent { kind, length });
        Ok(())
    }
    fn validate(&mut self, event: &TraceEvent, prior: &[TraceEvent]) -> Checked<()> {
        use IoTarget as T;
        use ParentKind as P;
        use TraceEvent as E;
        match event {
            E::ReadSubmit {
                request,
                offset,
                length,
            } => self.declare(*request, P::Read, *offset, *length)?,
            E::WriteSubmit {
                request,
                offset,
                data,
                ..
            } => self.declare(*request, P::Write, *offset, data.len() as u64)?,
            E::DiscardSubmit {
                request,
                offset,
                length,
            } => self.declare(*request, P::Discard, *offset, *length)?,
            E::FlushSubmit { request } => self.declare(*request, P::Flush, 0, 0)?,
            E::FillBegin { request, line } => {
                let length = self.line_length(*line)?;
                self.declare(*request, P::Fill, line * self.line_size, length)?;
            }
            E::IoSubmit {
                io,
                request,
                target,
                slot,
                source,
            } => {
                let p = self.parent(*request)?;
                if self.children.contains_key(io) {
                    return Err(TraceErrorCode::InvalidReference);
                }
                let compatible = matches!(
                    (p.kind, target),
                    (P::Read, T::OriginRead | T::CacheRead)
                        | (P::Write, T::OriginWrite)
                        | (P::Discard, T::OriginDiscard)
                        | (P::Flush, T::OriginFlush)
                        | (P::Fill, T::OriginRead | T::CacheWrite)
                );
                if !compatible {
                    return Err(TraceErrorCode::InvalidReference);
                }
                match target {
                    T::CacheRead => {
                        self.slot(slot.ok_or(TraceErrorCode::InvalidValue)?)?;
                        if source.is_some() {
                            return Err(TraceErrorCode::InvalidValue);
                        }
                        // Cross-line, unbound/currently wrong-line, lease and
                        // freshness conditions are deliberately NOT checked here.
                    }
                    T::CacheWrite => {
                        self.slot(slot.ok_or(TraceErrorCode::InvalidValue)?)?;
                        let src = self.child(source.ok_or(TraceErrorCode::InvalidValue)?)?;
                        if src.request != *request || src.target != T::OriginRead {
                            return Err(TraceErrorCode::InvalidReference);
                        }
                    }
                    _ => {
                        if slot.is_some() || source.is_some() {
                            return Err(TraceErrorCode::InvalidValue);
                        }
                    }
                }
                self.children.insert(
                    *io,
                    Child {
                        request: *request,
                        target: *target,
                        length: p.length,
                    },
                );
            }
            E::IoTransfer { io, offset, length } => {
                let c = self.child(*io)?;
                if c.target == T::OriginFlush {
                    return Err(TraceErrorCode::InvalidReference);
                }
                self.range(*offset, *length, c.length)?;
            }
            E::OriginPersist { io, offset, length } => {
                let c = self.child(*io)?;
                if !matches!(c.target, T::OriginWrite | T::OriginDiscard) {
                    return Err(TraceErrorCode::InvalidReference);
                }
                self.range(*offset, *length, c.length)?;
            }
            E::IoComplete { io, reported, .. } => {
                self.range(0, *reported, self.child(*io)?.length)?
            }
            E::CancelRequest { request } | E::RetireRequest { request } => {
                self.parent(*request)?;
            }
            E::CancelIo { io } => {
                self.child(*io)?;
            }
            E::CachePublish { io } => {
                if self.child(*io)?.target != T::CacheWrite {
                    return Err(TraceErrorCode::InvalidReference);
                }
            }
            E::ReadCommit { request, io } | E::MutationCommit { request, io } => {
                let p = self.parent(*request)?;
                let c = self.child(*io)?;
                let valid = if matches!(event, E::ReadCommit { .. }) {
                    p.kind == P::Read && matches!(c.target, T::OriginRead | T::CacheRead)
                } else {
                    matches!(
                        (p.kind, c.target),
                        (P::Write, T::OriginWrite)
                            | (P::Discard, T::OriginDiscard)
                            | (P::Flush, T::OriginFlush)
                    )
                };
                if c.request != *request || !valid {
                    return Err(TraceErrorCode::InvalidReference);
                }
            }
            E::SlotBind { slot, line } => {
                self.slot(*slot)?;
                self.line_length(*line)?;
            }
            E::AssertRead { request, data, .. } => {
                let p = self.parent(*request)?;
                if p.kind != P::Read {
                    return Err(TraceErrorCode::InvalidReference);
                }
                self.range(0, data.len() as u64, p.length)?;
            }
            E::AssertResult {
                request, reported, ..
            } => {
                let p = self.parent(*request)?;
                if p.kind == P::Fill {
                    return Err(TraceErrorCode::InvalidReference);
                }
                self.range(0, *reported, p.length)?;
            }
            E::AssertCache { slot, .. } => self.slot(*slot)?,
            E::AssertDecision { index, .. } => {
                let target = usize::try_from(*index)
                    .ok()
                    .and_then(|i| prior.get(i))
                    .ok_or(TraceErrorCode::InvalidReference)?;
                if target.is_assert() {
                    return Err(TraceErrorCode::InvalidReference);
                }
            }
            E::AssertOrigin { offset, data, .. } => {
                self.range(*offset, data.len() as u64, self.capacity)?
            }
            E::EpochInvalidate { .. } | E::ResumeCache | E::PowerLoss => {}
        }
        Ok(())
    }
}

pub fn parse_trace(input: &str) -> Result<TraceV1, TraceError> {
    let outer = |code| TraceError::new(code, None);
    if input.len() > TRACE_MAX_BYTES {
        return Err(outer(TraceErrorCode::InputTooLarge));
    }
    let Object(raw): Object<RawEnvelope<'_>> =
        serde_json::from_str(input).map_err(|e| json_error(e, None))?;
    if raw.schema_version.0 != 1 {
        return Err(outer(TraceErrorCode::InvalidSchema));
    }
    let seed = number(&raw.seed).map_err(outer)?;
    let capacity = number(&raw.capacity).map_err(outer)?;
    let counter_limit = identifier(&raw.counter_limit).map_err(outer)?;
    if capacity > TRACE_MAX_CAPACITY as u64 {
        return Err(outer(TraceErrorCode::LimitExceeded));
    }
    if !matches!(raw.line_size.0, 4096 | 8192 | 16384 | 32768 | 65536)
        || !(1..=256).contains(&raw.slot_count.0)
    {
        return Err(outer(TraceErrorCode::InvalidValue));
    }
    let EventSlices(slices) =
        serde_json::from_str(raw.events.get()).map_err(|e| json_error(e, None))?;
    let mut budget = PayloadBudget {
        used: 0,
        capacity: capacity as usize,
    };
    if hex_length(&raw.initial_hex).map_err(outer)? as u64 != capacity {
        return Err(outer(TraceErrorCode::InvalidValue));
    }
    let initial = budget.decode(&raw.initial_hex).map_err(outer)?;
    let mut references = StaticReferences {
        parents: BTreeMap::new(),
        children: BTreeMap::new(),
        capacity,
        line_size: raw.line_size.0,
        slot_count: raw.slot_count.0,
    };
    let mut events = Vec::with_capacity(slices.len());
    for (index, source) in slices.into_iter().enumerate() {
        let Object(event): Object<RawEvent> =
            serde_json::from_str(source.get()).map_err(|e| json_error(e, Some(index)))?;
        references
            .preflight_payload(&event)
            .map_err(|e| TraceError::new(e, Some(index)))?;
        let event = convert(event, &mut budget).map_err(|e| TraceError::new(e, Some(index)))?;
        references
            .validate(&event, &events)
            .map_err(|e| TraceError::new(e, Some(index)))?;
        events.push(event);
    }
    Ok(TraceV1 {
        seed,
        capacity,
        line_size: raw.line_size.0 as u32,
        slot_count: raw.slot_count.0 as u16,
        counter_limit,
        initial,
        events,
    })
}
