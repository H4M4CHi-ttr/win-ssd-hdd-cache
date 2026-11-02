//! D02 finite, host-only safety experiment. Not a cache engine or Windows model.
#![forbid(unsafe_code)]

use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Case {
    StaleFill,
    TwoWriters,
    PartialWrite,
    ShortFill,
    UnknownFill,
    HitRace,
    CounterExhaustion,
    LateSlotWrite,
    LateSlotRead,
    CancelledIo,
    LateCallerCompletion,
    StaleOverwrite,
    SupersededPublish,
}

impl Case {
    pub const ALL: [Self; 13] = [
        Self::StaleFill,
        Self::TwoWriters,
        Self::PartialWrite,
        Self::ShortFill,
        Self::UnknownFill,
        Self::HitRace,
        Self::CounterExhaustion,
        Self::LateSlotWrite,
        Self::LateSlotRead,
        Self::CancelledIo,
        Self::LateCallerCompletion,
        Self::StaleOverwrite,
        Self::SupersededPublish,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    None,
    IgnoreGeneration,
    CaptureTokenAtSubmit,
    BooleanWriters,
    PublishShortFill,
    PublishUnknownFill,
    SkipHitRevalidation,
    WrapGeneration,
    ReleaseOnEpoch,
    ReleaseOnCancel,
    CompleteParentTwice,
    KeepVisibilityOnCacheWrite,
    IgnoreContentProvenance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    SafeWithinBound,
    Violation,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Action {
    FillBegin,
    Sample(u8),
    FillSubmit,
    Transfer(u8),
    FillComplete(u8),
    Publish,
    WriterBegin(u8),
    Apply(u8, u8),
    WriterComplete(u8),
    HitBegin,
    HitSample(u8),
    HitComplete,
    HitCommit,
    OldIoBegin,
    EpochInvalidate,
    Cancel,
    Rebind,
    OldTransfer,
    OldComplete,
    CacheAttempt,
    Timeout,
    OriginComplete,
    OriginCommit,
    LateCacheComplete,
    SecondCapture,
    SecondSubmit,
    SecondTransfer(u8),
    SecondComplete,
    SecondPublish,
}

#[derive(Debug)]
pub struct Exploration {
    pub outcome: Outcome,
    pub states: usize,
    pub transitions: usize,
    pub terminals: usize,
    pub witness: Vec<Action>,
    pub violation: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct State {
    origin: [u8; 2],
    known: u8,
    generation: u8,
    counter_limit: u8,
    active: bool,
    writer_phase: [u8; 2],
    tracked_writers: u8,
    valid: bool,
    slot: [u8; 2],
    fill_began: bool,
    fill_generation: u8,
    fill_incarnation: u8,
    source: [u8; 2],
    sampled: u8,
    source_known: u8,
    fill_submitted: bool,
    transferred: u8,
    fill_complete: Option<u8>,
    hit_began: bool,
    hit_generation: u8,
    hit_data: [u8; 2],
    hit_sampled: u8,
    hit_complete: bool,
    hit_committed: bool,
    physical_io: bool,
    tracked_lease: bool,
    incarnation: u8,
    old_incarnation: u8,
    fallback_phase: u8,
    cache_pending: bool,
    caller_completions: u8,
    second_source: [u8; 2],
    second_generation: u8,
    second_incarnation: u8,
    second_transferred: u8,
    second_complete: bool,
}

impl State {
    fn new(case: Case) -> Self {
        Self {
            origin: [0; 2],
            known: if case == Case::UnknownFill { 1 } else { 3 },
            generation: 1,
            counter_limit: if case == Case::CounterExhaustion {
                2
            } else {
                4
            },
            active: true,
            writer_phase: [0; 2],
            tracked_writers: 0,
            valid: case == Case::HitRace,
            slot: if case == Case::HitRace {
                [0; 2]
            } else {
                [9; 2]
            },
            fill_began: false,
            fill_generation: 0,
            fill_incarnation: 0,
            source: [0; 2],
            sampled: 0,
            source_known: 0,
            fill_submitted: false,
            transferred: 0,
            fill_complete: None,
            hit_began: false,
            hit_generation: 0,
            hit_data: [0; 2],
            hit_sampled: 0,
            hit_complete: false,
            hit_committed: false,
            physical_io: false,
            tracked_lease: false,
            incarnation: 1,
            old_incarnation: 0,
            fallback_phase: 0,
            cache_pending: false,
            caller_completions: 0,
            second_source: [0; 2],
            second_generation: 0,
            second_incarnation: 0,
            second_transferred: 0,
            second_complete: false,
        }
    }

    fn actual_writers(&self) -> usize {
        self.writer_phase
            .iter()
            .filter(|&&phase| phase == 1)
            .count()
    }

    fn step(&mut self, action: Action, fault: Fault) -> Option<&'static str> {
        use Action::*;
        match action {
            FillBegin => {
                if self.active && self.tracked_writers == 0 {
                    self.fill_began = true;
                    self.fill_generation = self.generation;
                }
            }
            Sample(byte) if self.fill_began => {
                self.sampled |= 1 << byte;
                self.source[usize::from(byte)] = self.origin[usize::from(byte)];
                self.source_known |= self.known & (1 << byte);
            }
            FillSubmit if self.fill_began && self.sampled == 3 => {
                self.fill_submitted = true;
                if fault != Fault::KeepVisibilityOnCacheWrite {
                    self.valid = false;
                }
                // A new physical write changes content identity even when
                // the slot is still bound to the same line.
                self.incarnation += 1;
                self.fill_incarnation = self.incarnation;
                self.physical_io = true;
                if fault == Fault::CaptureTokenAtSubmit {
                    self.fill_generation = self.generation;
                }
            }
            Transfer(byte) if self.fill_submitted => {
                self.slot[usize::from(byte)] = self.source[usize::from(byte)];
                self.transferred |= 1 << byte;
            }
            FillComplete(bytes) if self.fill_submitted => {
                self.fill_complete = Some(bytes);
                self.physical_io = false;
            }
            Publish if self.fill_complete.is_some() => {
                let fresh =
                    self.fill_generation == self.generation || fault == Fault::IgnoreGeneration;
                let complete = (self.fill_complete == Some(2) && self.transferred == 3)
                    || fault == Fault::PublishShortFill;
                let known = self.source_known == 3 || fault == Fault::PublishUnknownFill;
                let current_content =
                    self.fill_incarnation == self.incarnation && !self.physical_io;
                if self.active
                    && self.tracked_writers == 0
                    && fresh
                    && complete
                    && known
                    && current_content
                {
                    self.valid = true;
                    // Independent ground truth uses admitted physical mutations,
                    // bytes/knownness/coverage, NOT the candidate's generation or count.
                    if self.actual_writers() != 0 {
                        return Some("published-with-live-writer");
                    }
                    if self.source_known != 3 {
                        return Some("published-unknown-source");
                    }
                    if self.transferred != 3 {
                        return Some("published-incomplete-slot");
                    }
                    if self.slot != self.origin {
                        return Some("published-stale-data");
                    }
                }
            }
            WriterBegin(writer) => {
                self.writer_phase[usize::from(writer)] = 1;
                self.valid = false;
                if self.generation == self.counter_limit {
                    if fault == Fault::WrapGeneration {
                        self.generation = 1;
                    } else {
                        self.active = false;
                    }
                } else {
                    self.generation += 1;
                }
                if fault == Fault::BooleanWriters {
                    self.tracked_writers = 1;
                } else {
                    self.tracked_writers += 1;
                }
            }
            Apply(writer, byte) => {
                self.origin[usize::from(byte)] = writer + 1;
                self.known |= 1 << byte;
            }
            WriterComplete(writer) => {
                self.writer_phase[usize::from(writer)] = 2;
                if fault == Fault::BooleanWriters {
                    self.tracked_writers = 0;
                } else {
                    self.tracked_writers -= 1;
                }
            }
            HitBegin if self.active && self.valid && self.tracked_writers == 0 => {
                self.hit_began = true;
                self.hit_generation = self.generation;
            }
            HitSample(byte) if self.hit_began => {
                self.hit_data[usize::from(byte)] = self.slot[usize::from(byte)];
                self.hit_sampled |= 1 << byte;
            }
            HitComplete if self.hit_began => {
                self.hit_complete = true;
            }
            HitCommit if self.hit_complete && self.hit_sampled == 3 => {
                let fresh = self.active
                    && self.valid
                    && self.hit_generation == self.generation
                    && self.tracked_writers == 0;
                if fresh || fault == Fault::SkipHitRevalidation {
                    self.hit_committed = true;
                    if self.actual_writers() != 0 {
                        return Some("hit-committed-with-live-writer");
                    }
                    if self.hit_data != self.origin {
                        return Some("hit-committed-stale-data");
                    }
                }
            }
            OldIoBegin => {
                self.physical_io = true;
                self.tracked_lease = true;
                self.old_incarnation = self.incarnation;
            }
            EpochInvalidate => {
                self.active = false;
                self.valid = false;
                if fault == Fault::ReleaseOnEpoch {
                    self.tracked_lease = false;
                }
            }
            Cancel => {
                if fault == Fault::ReleaseOnCancel {
                    self.tracked_lease = false;
                }
            }
            Rebind if !self.tracked_lease => {
                self.incarnation += 1;
            }
            OldTransfer if self.physical_io => {
                if self.incarnation != self.old_incarnation {
                    return Some("physical-io-after-slot-rebind");
                }
            }
            OldComplete => {
                self.physical_io = false;
                self.tracked_lease = false;
            }
            CacheAttempt => {
                self.cache_pending = true;
                self.fallback_phase = 1;
            }
            Timeout => {
                self.fallback_phase = 2;
            }
            OriginComplete => {
                self.fallback_phase = 3;
            }
            OriginCommit => {
                self.fallback_phase = 4;
                self.caller_completions += 1;
            }
            LateCacheComplete => {
                self.cache_pending = false;
                if fault == Fault::CompleteParentTwice {
                    self.caller_completions += 1;
                }
            }
            SecondCapture => {
                // Fixed two-fill witnesses capture the second source only
                // after the origin mutation has fully completed.
                self.second_source = self.origin;
                self.second_generation = self.generation;
            }
            SecondSubmit => {
                self.valid = false;
                self.incarnation += 1;
                self.second_incarnation = self.incarnation;
                self.physical_io = true;
            }
            SecondTransfer(byte) => {
                self.slot[usize::from(byte)] = self.second_source[usize::from(byte)];
                self.second_transferred |= 1 << byte;
            }
            SecondComplete => {
                self.second_complete = true;
                self.physical_io = false;
            }
            SecondPublish => {
                let current_content = (self.second_incarnation == self.incarnation
                    && !self.physical_io)
                    || fault == Fault::IgnoreContentProvenance;
                if self.second_complete
                    && self.second_transferred == 3
                    && current_content
                    && self.second_generation == self.generation
                    && self.active
                    && self.tracked_writers == 0
                {
                    self.valid = true;
                }
            }
            _ => {} // Continuation of an unadmitted transaction is a skipped step.
        }
        if self.caller_completions > 1 {
            return Some("double-caller-completion");
        }
        if self.valid
            && self.active
            && !self.physical_io
            && self.actual_writers() == 0
            && self.slot != self.origin
        {
            return Some("visible-cache-differs-from-origin");
        }
        None
    }
}

fn chains(case: Case) -> [Vec<Action>; 3] {
    use Action::*;
    let fill = vec![
        FillBegin,
        Sample(0),
        Sample(1),
        FillSubmit,
        Transfer(0),
        Transfer(1),
        FillComplete(2),
        Publish,
    ];
    let writer = |id| {
        vec![
            WriterBegin(id),
            Apply(id, 0),
            Apply(id, 1),
            WriterComplete(id),
        ]
    };
    match case {
        Case::StaleFill => [fill, writer(0), vec![]],
        Case::TwoWriters => [fill, writer(0), writer(1)],
        Case::PartialWrite => [
            fill,
            vec![WriterBegin(0), Apply(0, 0), WriterComplete(0)],
            vec![],
        ],
        Case::ShortFill => [
            vec![
                FillBegin,
                Sample(0),
                Sample(1),
                FillSubmit,
                Transfer(0),
                FillComplete(1),
                Publish,
            ],
            vec![],
            vec![],
        ],
        Case::UnknownFill => [fill, vec![], vec![]],
        Case::HitRace => [
            vec![HitBegin, HitSample(0), HitSample(1), HitComplete, HitCommit],
            writer(0),
            vec![],
        ],
        Case::CounterExhaustion => [
            fill,
            writer(0).into_iter().chain(writer(1)).collect(),
            vec![],
        ],
        Case::LateSlotWrite | Case::LateSlotRead => [
            vec![OldIoBegin, OldTransfer, OldComplete],
            vec![EpochInvalidate, Rebind],
            vec![],
        ],
        Case::CancelledIo => [
            vec![OldIoBegin, OldTransfer, OldComplete],
            vec![Cancel, Rebind],
            vec![],
        ],
        Case::LateCallerCompletion => [
            vec![CacheAttempt, Timeout, OriginComplete, OriginCommit],
            vec![LateCacheComplete],
            vec![],
        ],
        Case::StaleOverwrite | Case::SupersededPublish => {
            let mut sequence = vec![FillBegin, Sample(0), Sample(1)];
            sequence.extend(writer(0));
            sequence.extend([
                SecondCapture,
                SecondSubmit,
                SecondTransfer(0),
                SecondTransfer(1),
                SecondComplete,
            ]);
            if case == Case::StaleOverwrite {
                sequence.push(SecondPublish);
            }
            sequence.extend([
                FillSubmit,
                Transfer(0),
                Transfer(1),
                FillComplete(2),
                Publish,
            ]);
            if case == Case::SupersededPublish {
                sequence.push(SecondPublish);
            }
            [sequence, vec![], vec![]]
        }
    }
}

/// Breadth-first enumeration, deduplicating complete state + chain positions.
/// Fixed finite programs contain at most 18 steps, two writers, two byte cells,
/// two fills, one hit/slot and one fallback. CounterExhaustion uses counter_limit=2.
/// Two-fill witnesses fix program order and explore neither arbitrary fill
/// counts nor concurrent write leases. Incarnation is bounded to at most 3 here;
/// exhaustive content-counter overflow is a later replay obligation.
/// A max_states cutoff is explicitly inconclusive; no timer/random scheduler.
pub fn explore(case: Case, fault: Fault, max_states: usize) -> Exploration {
    let programs = chains(case);
    let start = State::new(case);
    let mut seen = HashSet::from([(start.clone(), [0_usize; 3])]);
    let mut queue = VecDeque::from([(start, [0_usize; 3], Vec::new())]);
    let mut report = Exploration {
        outcome: Outcome::SafeWithinBound,
        states: 1,
        transitions: 0,
        terminals: 0,
        witness: Vec::new(),
        violation: None,
    };
    if max_states == 0 {
        report.outcome = Outcome::Inconclusive;
        return report;
    }
    while let Some((state, positions, path)) = queue.pop_front() {
        if positions
            .iter()
            .zip(&programs)
            .all(|(p, program)| *p == program.len())
        {
            report.terminals += 1;
        }
        for lane in 0..programs.len() {
            let Some(&action) = programs[lane].get(positions[lane]) else {
                continue;
            };
            if action == Action::LateCacheComplete
                && (state.fallback_phase < 2 || !state.cache_pending)
            {
                continue;
            }
            let mut next = state.clone();
            let mut next_positions = positions;
            next_positions[lane] += 1;
            let mut witness = path.clone();
            witness.push(action);
            report.transitions += 1;
            if let Some(violation) = next.step(action, fault) {
                report.outcome = Outcome::Violation;
                report.witness = witness;
                report.violation = Some(violation);
                return report;
            }
            if seen.contains(&(next.clone(), next_positions)) {
                continue;
            }
            if seen.len() >= max_states {
                report.outcome = Outcome::Inconclusive;
                return report;
            }
            seen.insert((next.clone(), next_positions));
            report.states = seen.len();
            queue.push_back((next, next_positions, witness));
        }
    }
    report
}
