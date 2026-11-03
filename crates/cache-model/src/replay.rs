//! Deterministic host trace execution. This is not a cache engine or device API.
mod lower;
mod policy;
pub(crate) mod types;
use crate::oracle::Oracle;
use cache_types::{
    evidence::{Category, Evidence, Fault, Outcome, Terminal, Violation, ViolationCode as Code},
    trace::{Decision, TraceEvent, TraceV1},
};
use lower::Lower;
use policy::Policy;
use std::collections::BTreeMap;
pub use types::{CallerResult, DataCell, ResourceUsage};
use types::{Completion, RequestDescriptor, RequestKind, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayResult {
    evidence: Evidence,
    decisions: Vec<Option<Decision>>,
    results: BTreeMap<u64, CallerResult>,
    origin: Vec<DataCell>,
    usage: ResourceUsage,
}
impl ReplayResult {
    pub fn evidence(&self) -> &Evidence {
        &self.evidence
    }
    pub fn decision(&self, event: usize) -> Option<Decision> {
        self.decisions.get(event).copied().flatten()
    }
    pub fn decisions(&self) -> &[Option<Decision>] {
        &self.decisions
    }
    pub fn caller_result(&self, request: u64) -> Option<&CallerResult> {
        self.results.get(&request)
    }
    pub fn origin(&self) -> &[DataCell] {
        &self.origin
    }
    pub fn resource_usage(&self) -> ResourceUsage {
        self.usage
    }
}

struct Runner<'a> {
    lower: Lower<'a>,
    policy: Policy<'a>,
    oracle: Oracle<'a>,
    decisions: Vec<Option<Decision>>,
    declared_ios: BTreeMap<u64, u64>,
    power_lost: bool,
    abandoned_requests: u64,
    abandoned_ios: u64,
    line_size: usize,
    capacity: usize,
}

pub fn run_trace(trace: &TraceV1) -> ReplayResult {
    run_trace_with_fault(trace, Fault::None)
}

pub fn run_trace_with_fault(trace: &TraceV1, fault: Fault) -> ReplayResult {
    let mut runner = Runner {
        lower: Lower::new(
            trace.initial(),
            trace.line_size() as usize,
            trace.slot_count() as usize,
        ),
        policy: Policy::new(trace, fault),
        oracle: Oracle::new(
            trace.initial(),
            trace.line_size() as usize,
            trace.slot_count() as usize,
        ),
        decisions: Vec::new(),
        declared_ios: BTreeMap::new(),
        power_lost: false,
        abandoned_requests: 0,
        abandoned_ios: 0,
        line_size: trace.line_size() as usize,
        capacity: trace.initial().len(),
    };
    let mut violation = None;
    for event in trace.events() {
        let index = runner.decisions.len();
        runner.decisions.push(None);
        match runner.step(event) {
            Ok(decision) => runner.decisions[index] = decision,
            Err(code) => {
                violation = Some(Violation {
                    code,
                    event_index: index as u64,
                });
                break;
            }
        }
    }
    let pending_requests = if runner.power_lost {
        0
    } else {
        runner.policy.pending()
    };
    let pending_ios = if runner.power_lost {
        0
    } else {
        runner.lower.pending()
    };
    let terminal = if violation.is_some() {
        Terminal::Stopped
    } else if runner.power_lost {
        Terminal::PowerLoss
    } else if pending_requests > 0 || pending_ios > 0 {
        Terminal::Pending
    } else {
        Terminal::Drained
    };
    let meaningful = runner.oracle.checks[..4]
        .iter()
        .map(|c| c.passed + c.failed)
        .sum::<u64>();
    let outcome = if violation.is_some() {
        Outcome::Failed
    } else if terminal == Terminal::Pending || meaningful == 0 {
        Outcome::Inconclusive
    } else {
        Outcome::Passed
    };
    ReplayResult {
        evidence: Evidence {
            fault,
            seed: trace.seed(),
            source_revision: None,
            trace_digest: None,
            events_total: trace.events().len() as u64,
            events_executed: runner.decisions.len() as u64,
            outcome,
            terminal,
            pending_requests,
            pending_ios,
            abandoned_requests: runner.abandoned_requests,
            abandoned_ios: runner.abandoned_ios,
            violation,
            checks: runner.oracle.checks,
        },
        decisions: runner.decisions,
        results: runner.policy.results,
        usage: ResourceUsage {
            owned_cells: runner.policy.owned_cells,
            peak_owned_cells: runner.policy.peak_owned_cells,
            oracle_owned_cells: runner.oracle.owned_cells,
            peak_oracle_owned_cells: runner.oracle.peak_owned_cells,
            fixed_media_cells: runner.lower.origin.len() * 2
                + runner.lower.slots.iter().map(Vec::len).sum::<usize>(),
            oracle_fixed_media_cells: trace.initial().len() * 2
                + trace.line_size() as usize * trace.slot_count() as usize,
        },
        origin: runner.lower.origin,
    }
}

impl<'a> Runner<'a> {
    fn admit(
        &mut self,
        id: u64,
        kind: RequestKind,
        span: Span,
        data: &'a [u8],
        fua: bool,
    ) -> Decision {
        let descriptor = RequestDescriptor {
            id,
            span,
            kind,
            data,
            fua,
            admitted_at: self.decisions.len() - 1,
        };
        let decision = self.policy.admit(&self.lower, descriptor);
        self.oracle
            .admit(descriptor, decision == Decision::Accepted);
        decision
    }
    fn step(&mut self, event: &'a TraceEvent) -> Result<Option<Decision>, Code> {
        use TraceEvent::*;
        if self.power_lost && !event.is_assert() {
            return Err(Code::PostPowerLoss);
        }
        if let AssertRead { request, .. } | AssertResult { request, .. } = event
            && !self.policy.results.contains_key(request)
        {
            return Err(Code::InvalidPhase);
        }
        let index = self.decisions.len() - 1;
        let decision = match event {
            ReadSubmit {
                request,
                offset,
                length,
            } => {
                let span = Span {
                    start: *offset as usize,
                    len: *length as usize,
                };
                Some(self.admit(*request, RequestKind::Read, span, &[], false))
            }
            WriteSubmit {
                request,
                offset,
                data,
                fua,
            } => Some(self.admit(
                *request,
                RequestKind::Write,
                Span {
                    start: *offset as usize,
                    len: data.len(),
                },
                data,
                *fua,
            )),
            DiscardSubmit {
                request,
                offset,
                length,
            } => Some(self.admit(
                *request,
                RequestKind::Discard,
                Span {
                    start: *offset as usize,
                    len: *length as usize,
                },
                &[],
                false,
            )),
            FlushSubmit { request } => Some(self.admit(
                *request,
                RequestKind::Flush,
                Span { start: 0, len: 0 },
                &[],
                false,
            )),
            FillBegin { request, line } => {
                let start = *line as usize * self.line_size;
                Some(self.admit(
                    *request,
                    RequestKind::Fill,
                    Span {
                        start,
                        len: self.line_size.min(self.capacity - start),
                    },
                    &[],
                    false,
                ))
            }
            IoSubmit {
                io,
                request,
                target,
                slot,
                source,
            } => {
                let desc =
                    self.policy
                        .descriptor(&self.lower, *io, *request, *target, *slot, *source)?;
                self.declared_ios.insert(*io, *request);
                let decision = self.policy.admit_io(&self.lower, desc);
                if decision == Decision::Accepted {
                    self.oracle.forward(desc)?;
                    self.lower
                        .forward(desc, self.policy.parents[request].descriptor);
                }
                Some(decision)
            }
            IoTransfer { io, offset, length } => {
                self.lower
                    .transfer(*io, *offset as usize, *length as usize, index)?;
                self.oracle
                    .transfer(*io, *offset as usize, *length as usize, index);
                Some(Decision::Accepted)
            }
            IoComplete {
                io,
                status,
                reported,
            } => {
                let c = Completion {
                    status: *status,
                    reported: *reported,
                };
                self.lower.complete(*io, c)?;
                self.oracle.complete(*io, c)?;
                self.policy.complete(&self.lower, *io);
                Some(Decision::Accepted)
            }
            CachePublish { io } => {
                let d = self.policy.publish(&self.lower, *io)?;
                if d == Decision::Accepted {
                    self.oracle.publish(*io)?;
                }
                Some(d)
            }
            SlotBind { slot, line } => {
                let d = self
                    .policy
                    .bind(&self.lower, *slot as usize, *line as usize);
                if d == Decision::Accepted {
                    self.oracle.bind(*slot as usize, *line as usize, index)?;
                }
                Some(d)
            }
            EpochInvalidate { .. } => {
                self.policy.invalidate();
                self.oracle.invalidate(index);
                Some(Decision::Accepted)
            }
            ResumeCache => {
                let d = self.policy.resume(&self.lower);
                if d == Decision::Accepted {
                    self.oracle.resume()?;
                }
                Some(d)
            }
            ReadCommit { request, io } | MutationCommit { request, io } => {
                let d = self.policy.commit(&self.lower, *request, *io)?;
                if d == Decision::Accepted {
                    self.oracle
                        .commit(*request, *io, &self.policy.results[request])?;
                }
                Some(d)
            }
            OriginPersist { io, offset, length } => {
                self.lower
                    .persist(*io, *offset as usize, *length as usize)?;
                self.oracle.persist(
                    *io,
                    *offset as usize,
                    *length as usize,
                    &self.lower.durable,
                )?;
                Some(Decision::Accepted)
            }
            CancelRequest { request } => {
                self.policy.cancel_parent(*request)?;
                Some(Decision::Accepted)
            }
            CancelIo { io } => {
                let parent = self.declared_ios.get(io).ok_or(Code::InvalidPhase)?;
                if self.policy.parents[parent].retired {
                    return Err(Code::InvalidPhase);
                }
                if let Some(child) = self.lower.ios.get_mut(io) {
                    child.cancelled = true;
                }
                Some(Decision::Accepted)
            }
            RetireRequest { request } => {
                let decision = self.policy.retire(&self.lower, *request)?;
                if decision == Decision::Accepted {
                    self.oracle.retire(*request)?;
                    self.lower.retire(*request);
                }
                Some(decision)
            }
            PowerLoss => {
                self.abandoned_requests = self.policy.pending();
                self.abandoned_ios = self.lower.pending();
                self.power_lost = true;
                self.policy.power_loss();
                self.lower.power_loss();
                self.oracle.power_loss(&self.lower.origin)?;
                Some(Decision::Accepted)
            }
            AssertRead {
                request,
                data,
                known,
            } => {
                let matches = self
                    .policy
                    .results
                    .get(request)
                    .is_some_and(|r| r.data() == *data && r.known() == *known);
                self.oracle.count(
                    Category::TraceAssertions,
                    if matches {
                        Ok(())
                    } else {
                        Err(Code::AssertRead)
                    },
                )?;
                None
            }
            AssertResult {
                request,
                status,
                reported,
            } => {
                let matches = self
                    .policy
                    .results
                    .get(request)
                    .is_some_and(|r| r.status == *status && r.reported == *reported);
                self.oracle.count(
                    Category::TraceAssertions,
                    if matches {
                        Ok(())
                    } else {
                        Err(Code::AssertResult)
                    },
                )?;
                None
            }
            AssertCache { slot, valid } => {
                let matches = self.policy.visible(*slot as usize) == *valid;
                self.oracle.count(
                    Category::TraceAssertions,
                    if matches {
                        Ok(())
                    } else {
                        Err(Code::AssertCache)
                    },
                )?;
                None
            }
            AssertDecision { index, decision } => {
                let matches = self.decisions[*index as usize] == Some(*decision);
                self.oracle.count(
                    Category::TraceAssertions,
                    if matches {
                        Ok(())
                    } else {
                        Err(Code::AssertDecision)
                    },
                )?;
                None
            }
            AssertOrigin {
                offset,
                data,
                known,
            } => {
                let span = Span {
                    start: *offset as usize,
                    len: data.len(),
                };
                let matches = self.lower.origin[span.start..span.end()]
                    .iter()
                    .zip(data.iter().zip(known))
                    .all(|(c, (v, k))| c.value == *v && c.known == *k);
                self.oracle.count(
                    Category::TraceAssertions,
                    if matches {
                        Ok(())
                    } else {
                        Err(Code::AssertOrigin)
                    },
                )?;
                None
            }
        };
        if !event.is_assert() {
            self.oracle.retained_guard(&self.policy.results)?;
        }
        Ok(decision)
    }
}
