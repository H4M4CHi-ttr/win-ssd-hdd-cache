//! Serial metadata notifications, not kernel I/O ownership or synchronization.
use core::num::NonZeroU64;

use crate::{
    Completion, CompletionSnapshot, Submission, SubmitSnapshot, decode_completion, decode_submit,
};

const CONTEXT_CAPACITY: usize = 64;
const RECORD_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureState {
    Running,
    Stopping,
    Sealed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmitError {
    Closed,
    InvalidSnapshot,
    NoContext,
    IdentityExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompleteError {
    Closed,
    InvalidTicket,
    InvalidSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SealError {
    StillRunning,
    Outstanding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub state: CaptureState,
    pub active_contexts: usize,
    pub queued_records: usize,
    pub attempted: Option<u64>,
    pub dropped: Option<u64>,
    pub tracking_lost: bool,
    pub protocol_fault: bool,
    pub counter_exhausted: bool,
    pub identity_exhausted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Submit(Submission),
    Complete(Completion),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub seq: u64,
    pub session_id: u64,
    pub target_id: u64,
    pub request_id: u64,
    pub observation: Observation,
}

/// Valid only for its creating producer instance. It grants no physical lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    session_id: u64,
    target_id: u64,
    slot: usize,
    request_id: u64,
}

impl Ticket {
    pub const fn session_id(self) -> u64 {
        self.session_id
    }
    pub const fn target_id(self) -> u64 {
        self.target_id
    }
    pub const fn request_id(self) -> u64 {
        self.request_id
    }
}

#[derive(Clone, Copy)]
struct Context {
    request_id: u64,
    submission: Submission,
}

/// Fixed storage without allocation. A live producer cannot be cloned.
///
/// Caller-provided session IDs must be fresh across instances; do not carry a
/// ticket across recreation. The x64 size test is not a kernel stack budget.
pub struct Producer {
    session_id: u64,
    target_id: u64,
    state: CaptureState,
    contexts: [Option<Context>; CONTEXT_CAPACITY],
    active_contexts: usize,
    records: [Option<Record>; RECORD_CAPACITY],
    head: usize,
    queued_records: usize,
    attempted: Option<u64>,
    dropped: Option<u64>,
    next_request_id: Option<u64>,
    tracking_lost: bool,
    protocol_fault: bool,
    counter_exhausted: bool,
    identity_exhausted: bool,
}

impl Producer {
    pub fn new(session_id: NonZeroU64, target_id: NonZeroU64) -> Self {
        Self {
            session_id: session_id.get(),
            target_id: target_id.get(),
            state: CaptureState::Running,
            contexts: [None; CONTEXT_CAPACITY],
            active_contexts: 0,
            records: [None; RECORD_CAPACITY],
            head: 0,
            queued_records: 0,
            attempted: Some(0),
            dropped: Some(0),
            next_request_id: Some(1),
            tracking_lost: false,
            protocol_fault: false,
            counter_exhausted: false,
            identity_exhausted: false,
        }
    }

    pub fn submit(&mut self, raw: SubmitSnapshot) -> Result<Ticket, SubmitError> {
        if self.state != CaptureState::Running {
            return Err(SubmitError::Closed);
        }
        let submission = decode_submit(raw).map_err(|_| {
            self.protocol_fault = true;
            self.tracking_lost = true;
            SubmitError::InvalidSnapshot
        })?;
        let seq = self.attempt();
        let Some(slot) = self.contexts.iter().position(Option::is_none) else {
            self.drop_event();
            self.tracking_lost = true;
            return Err(SubmitError::NoContext);
        };
        let Some(request_id) = self.next_request_id else {
            self.drop_event();
            self.tracking_lost = true;
            return Err(SubmitError::IdentityExhausted);
        };
        self.next_request_id = request_id.checked_add(1);
        if self.next_request_id.is_none() {
            self.identity_exhausted = true;
        }
        self.contexts[slot] = Some(Context {
            request_id,
            submission,
        });
        self.active_contexts += 1;
        self.record(seq, request_id, Observation::Submit(submission));
        Ok(Ticket {
            session_id: self.session_id,
            target_id: self.target_id,
            slot,
            request_id,
        })
    }

    pub fn complete(
        &mut self,
        ticket: Ticket,
        raw: CompletionSnapshot,
    ) -> Result<(), CompleteError> {
        if self.state == CaptureState::Sealed {
            return Err(CompleteError::Closed);
        }
        let matched = self
            .contexts
            .get(ticket.slot)
            .copied()
            .flatten()
            .filter(|context| {
                ticket.session_id == self.session_id
                    && ticket.target_id == self.target_id
                    && ticket.request_id == context.request_id
            });
        let Some(context) = matched else {
            self.protocol_fault = true;
            self.tracking_lost = true;
            return Err(CompleteError::InvalidTicket);
        };
        let seq = self.attempt();
        // This retires observer metadata on the caller's completion notification,
        // even when its scalar snapshot is invalid. It releases no IRP or buffer.
        self.contexts[ticket.slot] = None;
        self.active_contexts -= 1;
        let completion = decode_completion(raw, &context.submission).map_err(|_| {
            self.drop_event();
            self.protocol_fault = true;
            CompleteError::InvalidSnapshot
        })?;
        self.record(seq, context.request_id, Observation::Complete(completion));
        Ok(())
    }

    pub fn stop(&mut self) {
        if self.state == CaptureState::Running {
            self.state = CaptureState::Stopping;
        }
    }

    /// Seals only tracked metadata. Never a certificate of Windows I/O drain.
    pub fn seal(&mut self) -> Result<Stats, SealError> {
        if self.state == CaptureState::Running {
            return Err(SealError::StillRunning);
        }
        if self.active_contexts != 0 {
            return Err(SealError::Outstanding);
        }
        self.state = CaptureState::Sealed;
        Ok(self.stats())
    }

    pub fn pop(&mut self) -> Option<Record> {
        if self.queued_records == 0 {
            return None;
        }
        let record = self.records[self.head].take();
        self.head = (self.head + 1) % RECORD_CAPACITY;
        self.queued_records -= 1;
        record
    }

    pub const fn stats(&self) -> Stats {
        Stats {
            state: self.state,
            active_contexts: self.active_contexts,
            queued_records: self.queued_records,
            attempted: self.attempted,
            dropped: self.dropped,
            tracking_lost: self.tracking_lost,
            protocol_fault: self.protocol_fault,
            counter_exhausted: self.counter_exhausted,
            identity_exhausted: self.identity_exhausted,
        }
    }

    fn exhaust_counters(&mut self) {
        self.attempted = None;
        self.dropped = None;
        self.counter_exhausted = true;
    }

    fn attempt(&mut self) -> Option<u64> {
        let seq = self.attempted?;
        match seq.checked_add(1) {
            Some(next) => {
                self.attempted = Some(next);
                Some(seq)
            }
            None => {
                self.exhaust_counters();
                None
            }
        }
    }

    fn drop_event(&mut self) {
        if let Some(dropped) = self.dropped {
            match dropped.checked_add(1) {
                Some(next) => self.dropped = Some(next),
                None => self.exhaust_counters(),
            }
        }
    }

    fn record(&mut self, seq: Option<u64>, request_id: u64, observation: Observation) {
        let Some(seq) = seq else {
            return;
        };
        if self.queued_records == RECORD_CAPACITY {
            self.drop_event();
            return;
        }
        let slot = (self.head + self.queued_records) % RECORD_CAPACITY;
        self.records[slot] = Some(Record {
            seq,
            session_id: self.session_id,
            target_id: self.target_id,
            request_id,
            observation,
        });
        self.queued_records += 1;
    }
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod limits_tests;
