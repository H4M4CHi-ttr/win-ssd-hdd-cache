//! Independent history oracle. No reference metadata/generations or policy helper calls.
use crate::replay::types::{
    CallerResult, Completion, DataCell, IoDescriptor, MAX_LIVE, MAX_OWNED_CELLS, RequestDescriptor,
    RequestKind, Span,
};
use cache_types::{
    evidence::{Category, Check, ViolationCode as Code},
    trace::{IoStatus, IoTarget},
};
use std::collections::BTreeMap;

struct ObservedParent<'a> {
    descriptor: RequestDescriptor<'a>,
    admitted: bool,
    retired: bool,
    fill_admission_safe: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Publication {
    write: u64,
    fill: u64,
    binding_at: usize,
}
struct ObservedIo {
    descriptor: IoDescriptor,
    samples: Vec<DataCell>,
    completion: Option<Completion>,
    binding_at: usize,
    lineage: Option<Publication>,
    published: bool,
}
struct ObservedSlot {
    line: Option<usize>,
    binding_at: usize,
    owner: Option<u64>,
    publication: Option<Publication>,
    cells: Vec<DataCell>,
}
pub(crate) struct Oracle<'a> {
    expected_origin: Vec<DataCell>,
    expected_durable: Vec<DataCell>,
    parents: BTreeMap<u64, ObservedParent<'a>>,
    ios: BTreeMap<u64, ObservedIo>,
    immutable_results: BTreeMap<u64, CallerResult>,
    slots: Vec<ObservedSlot>,
    line_size: usize,
    active: bool,
    last_invalidation: Option<usize>,
    pub checks: [Check; 6],
    pub owned_cells: usize,
    pub peak_owned_cells: usize,
}
impl<'a> Oracle<'a> {
    pub fn new(initial: &[u8], line_size: usize, slots: usize) -> Self {
        Self {
            expected_origin: initial.iter().copied().map(DataCell::initial).collect(),
            expected_durable: initial.iter().copied().map(DataCell::initial).collect(),
            parents: BTreeMap::new(),
            ios: BTreeMap::new(),
            immutable_results: BTreeMap::new(),
            slots: (0..slots)
                .map(|_| ObservedSlot {
                    line: None,
                    binding_at: 0,
                    owner: None,
                    publication: None,
                    cells: vec![DataCell::default(); line_size],
                })
                .collect(),
            line_size,
            active: true,
            last_invalidation: None,
            checks: [Check::default(); 6],
            owned_cells: 0,
            peak_owned_cells: 0,
        }
    }
    pub fn count(&mut self, category: Category, result: Result<(), Code>) -> Result<(), Code> {
        let check = &mut self.checks[category.index()];
        if result.is_ok() {
            check.passed += 1;
        } else {
            check.failed += 1;
        }
        result
    }
    fn overlap(a: Span, b: Span) -> bool {
        a.len != 0 && b.len != 0 && a.start < b.end() && b.start < a.end()
    }
    fn outstanding_writer(&self, span: Span) -> bool {
        self.parents.values().any(|p| {
            p.admitted
                && matches!(p.descriptor.kind, RequestKind::Write | RequestKind::Discard)
                && Self::overlap(p.descriptor.span, span)
                && !self
                    .ios
                    .values()
                    .any(|io| io.descriptor.parent == p.descriptor.id && io.completion.is_some())
        })
    }
    pub fn admit(&mut self, descriptor: RequestDescriptor<'a>, admitted: bool) {
        let fill_admission_safe = self.active && !self.outstanding_writer(descriptor.span);
        if admitted {
            if !matches!(descriptor.kind, RequestKind::Fill | RequestKind::Flush) {
                self.owned_cells += descriptor.span.len;
                self.peak_owned_cells = self.peak_owned_cells.max(self.owned_cells);
            }
            if matches!(descriptor.kind, RequestKind::Write | RequestKind::Discard) {
                for slot in &mut self.slots {
                    if let Some(line) = slot.line {
                        let extent = Span {
                            start: line * self.line_size,
                            len: self
                                .line_size
                                .min(self.expected_origin.len() - line * self.line_size),
                        };
                        if Self::overlap(descriptor.span, extent) {
                            slot.publication = None;
                        }
                    }
                }
            }
        }
        self.parents.insert(
            descriptor.id,
            ObservedParent {
                descriptor,
                admitted,
                retired: false,
                fill_admission_safe,
            },
        );
    }
    fn lease_conflict(&self, slot: u64, exclusive: bool) -> bool {
        self.ios.values().any(|io| {
            io.descriptor.slot == Some(slot)
                && io.completion.is_none()
                && (exclusive || io.descriptor.target == IoTarget::CacheWrite)
        })
    }
    fn full_success(io: &ObservedIo) -> bool {
        io.completion
            == Some(Completion {
                status: IoStatus::Ok,
                reported: io.descriptor.span.len as u64,
            })
            && io.samples.len() == io.descriptor.span.len
            && io.samples.iter().all(|c| c.covered && c.known)
    }
    fn source_still_authoritative(&self, fill: u64) -> bool {
        let Some(p) = self.parents.get(&fill) else {
            return false;
        };
        p.admitted
            && p.descriptor.kind == RequestKind::Fill
            && p.fill_admission_safe
            && self.active
            && self
                .last_invalidation
                .is_none_or(|i| i < p.descriptor.admitted_at)
            && !self.parents.values().any(|m| {
                m.admitted
                    && matches!(m.descriptor.kind, RequestKind::Write | RequestKind::Discard)
                    && m.descriptor.admitted_at > p.descriptor.admitted_at
                    && Self::overlap(m.descriptor.span, p.descriptor.span)
            })
            && !self.outstanding_writer(p.descriptor.span)
    }
    fn hit_is_authoritative(&self, slot_id: usize, span: Span, lineage: Publication) -> bool {
        let slot = &self.slots[slot_id];
        if span.start >= self.expected_origin.len()
            || (span.len != 0 && span.start / self.line_size != (span.end() - 1) / self.line_size)
        {
            return false;
        }
        slot.line == Some(span.start / self.line_size)
            && slot.publication == Some(lineage)
            && slot.owner == Some(lineage.write)
            && slot.binding_at == lineage.binding_at
            && self.source_still_authoritative(lineage.fill)
            && slot.cells[span.start % self.line_size..span.start % self.line_size + span.len]
                .iter()
                .all(|c| c.covered && c.known)
    }
    fn forwarding_is_valid(&self, d: IoDescriptor) -> Result<(), Code> {
        let p = self.parents.get(&d.parent).ok_or(Code::InvalidOwnership)?;
        let kind_matches = matches!(
            (p.descriptor.kind, d.target),
            (
                RequestKind::Read,
                IoTarget::OriginRead | IoTarget::CacheRead
            ) | (
                RequestKind::Fill,
                IoTarget::OriginRead | IoTarget::CacheWrite
            ) | (RequestKind::Write, IoTarget::OriginWrite)
                | (RequestKind::Discard, IoTarget::OriginDiscard)
                | (RequestKind::Flush, IoTarget::OriginFlush)
        );
        if !p.admitted
            || p.retired
            || self.immutable_results.contains_key(&d.parent)
            || p.descriptor.span != d.span
            || !kind_matches
            || self
                .ios
                .values()
                .any(|io| io.descriptor.parent == d.parent && io.descriptor.target == d.target)
        {
            return Err(Code::InvalidOwnership);
        }
        if matches!(d.target, IoTarget::CacheRead | IoTarget::CacheWrite) {
            let slot_id = d.slot.ok_or(Code::InvalidOwnership)?;
            let slot = self
                .slots
                .get(slot_id as usize)
                .ok_or(Code::InvalidOwnership)?;
            if self.lease_conflict(slot_id, d.target == IoTarget::CacheWrite) {
                return Err(Code::LiveLeaseReused);
            }
            if d.target == IoTarget::CacheWrite {
                let source = self
                    .ios
                    .get(&d.source.ok_or(Code::InvalidOwnership)?)
                    .ok_or(Code::InvalidOwnership)?;
                if slot.line != Some(d.span.start / self.line_size)
                    || source.descriptor.parent != d.parent
                    || source.descriptor.target != IoTarget::OriginRead
                    || source.descriptor.span != d.span
                    || !Self::full_success(source)
                {
                    return Err(Code::InvalidOwnership);
                }
            } else if d.source.is_some() {
                return Err(Code::InvalidOwnership);
            }
        } else if d.slot.is_some() || d.source.is_some() {
            return Err(Code::InvalidOwnership);
        }
        if d.target == IoTarget::OriginFlush {
            for mutation in self.parents.values().filter(|m| {
                m.admitted
                    && m.descriptor.admitted_at < p.descriptor.admitted_at
                    && matches!(m.descriptor.kind, RequestKind::Write | RequestKind::Discard)
            }) {
                if !self.ios.values().any(|io| {
                    io.descriptor.parent == mutation.descriptor.id && io.completion.is_some()
                }) {
                    return Err(Code::InvalidOwnership);
                }
            }
        }
        let needed = if matches!(
            d.target,
            IoTarget::OriginRead | IoTarget::CacheRead | IoTarget::CacheWrite
        ) {
            d.span.len
        } else {
            0
        };
        let reserved_others = self
            .parents
            .values()
            .filter(|p| {
                p.admitted
                    && p.descriptor.id != d.parent
                    && matches!(p.descriptor.kind, RequestKind::Write | RequestKind::Discard)
                    && !self
                        .ios
                        .values()
                        .any(|io| io.descriptor.parent == p.descriptor.id)
            })
            .count();
        if !self
            .owned_cells
            .checked_add(needed)
            .is_some_and(|n| n <= MAX_OWNED_CELLS)
            || self
                .ios
                .values()
                .filter(|io| io.completion.is_none())
                .count()
                + reserved_others
                >= MAX_LIVE
        {
            return Err(Code::InvalidOwnership);
        }
        Ok(())
    }
    pub fn forward(&mut self, descriptor: IoDescriptor) -> Result<(), Code> {
        self.count(Category::Ownership, self.forwarding_is_valid(descriptor))?;
        let mut lineage = None;
        let mut binding_at = 0;
        if let Some(slot) = descriptor.slot {
            binding_at = self.slots[slot as usize].binding_at;
            if descriptor.target == IoTarget::CacheRead {
                lineage = self.slots[slot as usize].publication;
                let valid = lineage
                    .is_some_and(|l| self.hit_is_authoritative(slot as usize, descriptor.span, l));
                self.count(
                    Category::CacheLineage,
                    if valid {
                        Ok(())
                    } else {
                        Err(Code::StaleCacheResult)
                    },
                )?;
            } else {
                self.slots[slot as usize].owner = Some(descriptor.id);
                self.slots[slot as usize].publication = None;
            }
        }
        if matches!(
            descriptor.target,
            IoTarget::OriginRead | IoTarget::CacheRead | IoTarget::CacheWrite
        ) {
            self.owned_cells += descriptor.span.len;
            self.peak_owned_cells = self.peak_owned_cells.max(self.owned_cells);
        }
        self.ios.insert(
            descriptor.id,
            ObservedIo {
                descriptor,
                samples: vec![DataCell::default(); descriptor.span.len],
                completion: None,
                binding_at,
                lineage,
                published: false,
            },
        );
        Ok(())
    }
    pub fn transfer(&mut self, id: u64, offset: usize, len: usize, event: usize) {
        let mut io = self.ios.remove(&id).expect("accepted forward recorded");
        let parent = &self.parents[&io.descriptor.parent].descriptor;
        for relative in offset..offset + len {
            let absolute = io.descriptor.span.start + relative;
            // Independent expected state from input/history, never the actual lower buffer.
            let cell = match io.descriptor.target {
                IoTarget::OriginRead => self.expected_origin[absolute],
                IoTarget::OriginWrite => DataCell {
                    version: event as u64 + 1,
                    value: parent.data[relative],
                    known: true,
                    covered: true,
                },
                IoTarget::OriginDiscard => DataCell {
                    version: event as u64 + 1,
                    value: 0,
                    known: false,
                    covered: true,
                },
                IoTarget::CacheWrite => {
                    self.ios[&io.descriptor.source.expect("checked source")].samples[relative]
                }
                IoTarget::CacheRead => {
                    self.slots[io.descriptor.slot.expect("checked slot") as usize].cells
                        [absolute % self.line_size]
                }
                IoTarget::OriginFlush => unreachable!("F05 rejects flush transfer"),
            };
            io.samples[relative] = cell;
            match io.descriptor.target {
                IoTarget::OriginWrite | IoTarget::OriginDiscard => {
                    self.expected_origin[absolute] = cell
                }
                IoTarget::CacheWrite => {
                    self.slots[io.descriptor.slot.expect("slot") as usize].cells[relative] = cell
                }
                _ => {}
            }
        }
        self.ios.insert(id, io);
    }
    fn covered_by_durable(&self, io: &ObservedIo, prefix: usize) -> bool {
        io.samples[..prefix].iter().enumerate().all(|(i, cell)| {
            !cell.covered
                || self.expected_durable[io.descriptor.span.start + i].version >= cell.version
        })
    }
    pub fn complete(&mut self, id: u64, completion: Completion) -> Result<(), Code> {
        let io = self.ios.get_mut(&id).expect("accepted forward recorded");
        io.completion = Some(completion);
        if matches!(
            io.descriptor.target,
            IoTarget::CacheRead | IoTarget::CacheWrite
        ) {
            return Ok(());
        }
        let covered = io
            .samples
            .get(..completion.reported as usize)
            .is_some_and(|s| s.iter().all(|c| c.covered));
        let code = if io.descriptor.target == IoTarget::OriginRead {
            Code::ReadReportedUnsampled
        } else {
            Code::WriteReportedUnapplied
        };
        self.count(
            Category::OriginResults,
            if covered { Ok(()) } else { Err(code) },
        )?;
        let io = &self.ios[&id];
        let parent = &self.parents[&io.descriptor.parent].descriptor;
        if completion.status == IoStatus::Ok
            && parent.fua
            && io.descriptor.target == IoTarget::OriginWrite
        {
            let covered = self.covered_by_durable(io, completion.reported as usize);
            self.count(
                Category::Durability,
                if covered {
                    Ok(())
                } else {
                    Err(Code::FuaNotDurable)
                },
            )?;
        } else if completion.status == IoStatus::Ok && io.descriptor.target == IoTarget::OriginFlush
        {
            let covered = self
                .ios
                .values()
                .filter(|io| {
                    let p = &self.parents[&io.descriptor.parent];
                    p.admitted
                        && p.descriptor.admitted_at < parent.admitted_at
                        && matches!(
                            io.descriptor.target,
                            IoTarget::OriginWrite | IoTarget::OriginDiscard
                        )
                })
                .all(|io| self.covered_by_durable(io, io.samples.len()));
            self.count(
                Category::Durability,
                if covered {
                    Ok(())
                } else {
                    Err(Code::FlushNotDurable)
                },
            )?;
        }
        Ok(())
    }
    pub fn persist(
        &mut self,
        id: u64,
        offset: usize,
        len: usize,
        actual: &[DataCell],
    ) -> Result<(), Code> {
        let io = &self.ios[&id];
        for relative in offset..offset + len {
            let incoming = io.samples[relative];
            let durable = &mut self.expected_durable[io.descriptor.span.start + relative];
            if incoming.version > durable.version {
                *durable = incoming;
            }
        }
        if len != 0 {
            self.count(
                Category::Durability,
                if actual == self.expected_durable {
                    Ok(())
                } else {
                    Err(Code::DurableHistoryMismatch)
                },
            )?;
        }
        Ok(())
    }
    pub fn power_loss(&mut self, actual: &[DataCell]) -> Result<(), Code> {
        self.expected_origin.copy_from_slice(&self.expected_durable);
        self.active = false;
        for slot in &mut self.slots {
            slot.publication = None;
            slot.cells.fill(DataCell::default());
        }
        let has_mutation = self.ios.values().any(|io| {
            matches!(
                io.descriptor.target,
                IoTarget::OriginWrite | IoTarget::OriginDiscard
            ) && io.samples.iter().any(|c| c.covered)
        });
        if has_mutation {
            self.count(
                Category::Durability,
                if actual == self.expected_durable {
                    Ok(())
                } else {
                    Err(Code::DurableHistoryMismatch)
                },
            )?;
        }
        Ok(())
    }
    fn publication_is_valid(&self, io: &ObservedIo) -> bool {
        let Some(slot) = io
            .descriptor
            .slot
            .and_then(|id| self.slots.get(id as usize))
        else {
            return false;
        };
        let Some(source) = io.descriptor.source.and_then(|id| self.ios.get(&id)) else {
            return false;
        };
        self.source_still_authoritative(io.descriptor.parent)
            && Self::full_success(io)
            && Self::full_success(source)
            && !io.published
            && slot.owner == Some(io.descriptor.id)
            && slot.binding_at == io.binding_at
            && slot.line == Some(io.descriptor.span.start / self.line_size)
            && !self.lease_conflict(io.descriptor.slot.expect("checked slot"), false)
    }
    pub fn publish(&mut self, id: u64) -> Result<(), Code> {
        let io = &self.ios[&id];
        let valid = io.descriptor.target == IoTarget::CacheWrite
            && !self.parents[&io.descriptor.parent].retired
            && self.publication_is_valid(io);
        self.count(
            Category::CacheLineage,
            if valid {
                Ok(())
            } else {
                Err(Code::StalePublication)
            },
        )?;
        let io = self.ios.get_mut(&id).expect("checked IO");
        let publication = Publication {
            write: id,
            fill: io.descriptor.parent,
            binding_at: io.binding_at,
        };
        io.published = true;
        self.slots[io.descriptor.slot.expect("checked slot") as usize].publication =
            Some(publication);
        Ok(())
    }
    pub fn commit(&mut self, request: u64, id: u64, claim: &CallerResult) -> Result<(), Code> {
        let io = self.ios.get(&id).expect("accepted forward recorded");
        let is_read = matches!(
            io.descriptor.target,
            IoTarget::OriginRead | IoTarget::CacheRead
        );
        let cache_read = io.descriptor.target == IoTarget::CacheRead;
        let category = if cache_read {
            Category::CacheLineage
        } else {
            Category::OriginResults
        };
        let result = if io.descriptor.parent != request
            || self
                .parents
                .get(&request)
                .is_none_or(|p| !p.admitted || p.retired || p.descriptor.kind == RequestKind::Fill)
        {
            Err(Code::InvalidOwnership)
        } else if self.immutable_results.contains_key(&request) {
            Err(Code::DuplicateResult)
        } else if cache_read
            && (!Self::full_success(io)
                || !io.lineage.is_some_and(|l| {
                    self.hit_is_authoritative(
                        io.descriptor.slot.expect("cache slot") as usize,
                        io.descriptor.span,
                        l,
                    )
                }))
        {
            Err(Code::StaleCacheResult)
        } else if let Some(c) = io.completion {
            let expected = if is_read {
                &io.samples[..c.reported as usize]
            } else {
                &[]
            };
            if c.status != claim.status || c.reported != claim.reported || claim.cells != expected {
                Err(Code::LowerResultChanged)
            } else {
                Ok(())
            }
        } else {
            Err(Code::EarlyAck)
        };
        self.count(category, result)?;
        let io = &self.ios[&id];
        let c = io
            .completion
            .expect("successful check requires actual completion");
        if is_read {
            self.owned_cells -= io.descriptor.span.len - c.reported as usize;
        }
        self.immutable_results.insert(
            request,
            CallerResult {
                status: c.status,
                reported: c.reported,
                cells: if is_read {
                    io.samples[..c.reported as usize].to_vec()
                } else {
                    Vec::new()
                },
            },
        );
        Ok(())
    }
    pub fn bind(&mut self, slot: usize, line: usize, event: usize) -> Result<(), Code> {
        self.count(
            Category::Ownership,
            if self.lease_conflict(slot as u64, true) {
                Err(Code::LiveLeaseReused)
            } else {
                Ok(())
            },
        )?;
        let slot = &mut self.slots[slot];
        slot.line = Some(line);
        slot.binding_at = event;
        slot.owner = None;
        slot.publication = None;
        Ok(())
    }
    pub fn invalidate(&mut self, event: usize) {
        self.active = false;
        self.last_invalidation = Some(event);
        for slot in &mut self.slots {
            slot.publication = None;
        }
    }
    pub fn resume(&mut self) -> Result<(), Code> {
        let allowed = self.active
            || (self.ios.values().all(|io| io.completion.is_some())
                && self
                    .parents
                    .values()
                    .filter(|p| {
                        p.admitted
                            && matches!(
                                p.descriptor.kind,
                                RequestKind::Write | RequestKind::Discard
                            )
                    })
                    .all(|p| {
                        self.ios.values().any(|io| {
                            io.descriptor.parent == p.descriptor.id && io.completion.is_some()
                        })
                    }));
        self.count(
            Category::Ownership,
            if allowed {
                Ok(())
            } else {
                Err(Code::InvalidOwnership)
            },
        )?;
        if !self.active {
            self.active = true;
            for slot in &mut self.slots {
                slot.publication = None;
            }
        }
        Ok(())
    }
    pub fn retire(&mut self, request: u64) -> Result<(), Code> {
        let valid = self.parents.get(&request).is_some_and(|p| {
            !p.retired
                && (!p.admitted
                    || p.descriptor.kind == RequestKind::Fill
                    || self.immutable_results.contains_key(&request))
        }) && self
            .ios
            .values()
            .filter(|io| io.descriptor.parent == request)
            .all(|io| io.completion.is_some());
        self.count(
            Category::Ownership,
            if valid {
                Ok(())
            } else {
                Err(Code::InvalidOwnership)
            },
        )?;
        self.parents
            .get_mut(&request)
            .expect("checked parent")
            .retired = true;
        for io in self
            .ios
            .values_mut()
            .filter(|io| io.descriptor.parent == request)
        {
            if matches!(
                io.descriptor.target,
                IoTarget::OriginRead | IoTarget::CacheRead | IoTarget::CacheWrite
            ) {
                self.owned_cells -= io.samples.len();
                io.samples = Vec::new();
            }
        }
        Ok(())
    }
    pub fn retained_guard(&mut self, claims: &BTreeMap<u64, CallerResult>) -> Result<(), Code> {
        if self.immutable_results.is_empty() && claims.is_empty() {
            return Ok(());
        }
        self.count(
            Category::Ownership,
            if claims == &self.immutable_results {
                Ok(())
            } else {
                Err(Code::ResultMutated)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_claim() -> CallerResult {
        CallerResult {
            status: IoStatus::IoError,
            reported: 1,
            cells: vec![DataCell::initial(65)],
        }
    }

    fn completed_origin() -> Oracle<'static> {
        let mut oracle = Oracle::new(&[65], 4096, 1);
        let span = Span { start: 0, len: 1 };
        oracle.admit(
            RequestDescriptor {
                id: 1,
                span,
                kind: RequestKind::Read,
                data: &[],
                fua: false,
                admitted_at: 0,
            },
            true,
        );
        oracle
            .forward(IoDescriptor {
                id: 1,
                parent: 1,
                target: IoTarget::OriginRead,
                span,
                slot: None,
                source: None,
            })
            .unwrap();
        oracle.transfer(1, 0, 1, 2);
        oracle
            .complete(
                1,
                Completion {
                    status: IoStatus::IoError,
                    reported: 1,
                },
            )
            .unwrap();
        oracle
    }

    #[test]
    fn origin_claim_cannot_change_status_count_bytes_knownness_or_history() {
        for corruption in 0..6 {
            let mut oracle = completed_origin();
            let mut claim = read_claim();
            match corruption {
                0 => claim.status = IoStatus::Ok,
                1 => claim.reported = 0,
                2 => claim.cells[0].value = 66,
                3 => claim.cells[0].known = false,
                4 => claim.cells[0].version = 1,
                _ => claim.cells.clear(),
            }
            assert_eq!(oracle.commit(1, 1, &claim), Err(Code::LowerResultChanged));
            assert_eq!(oracle.checks[Category::OriginResults.index()].failed, 1);
            assert!(oracle.immutable_results.is_empty());
        }
    }

    #[test]
    fn duplicate_result_is_a_safety_failure_not_a_second_snapshot() {
        let mut oracle = completed_origin();
        oracle.commit(1, 1, &read_claim()).unwrap();
        assert_eq!(
            oracle.commit(1, 1, &read_claim()),
            Err(Code::DuplicateResult)
        );
        assert_eq!(oracle.immutable_results.len(), 1);
        assert_eq!(oracle.checks[Category::OriginResults.index()].passed, 2);
        assert_eq!(oracle.checks[Category::OriginResults.index()].failed, 1);
    }

    #[test]
    fn retained_guard_checks_missing_extra_and_mutated_results_once_per_collection() {
        for corruption in 0..7 {
            let mut oracle = completed_origin();
            let mut claims = BTreeMap::from([(1, read_claim())]);
            oracle.commit(1, 1, &claims[&1]).unwrap();
            let before = oracle.checks[Category::Ownership.index()].passed;
            oracle.retained_guard(&claims).unwrap();
            assert_eq!(
                oracle.checks[Category::Ownership.index()].passed,
                before + 1
            );
            match corruption {
                0 => claims.clear(),
                1 => {
                    claims.insert(2, read_claim());
                }
                2 => claims.get_mut(&1).unwrap().status = IoStatus::Ok,
                3 => claims.get_mut(&1).unwrap().reported = 0,
                4 => claims.get_mut(&1).unwrap().cells[0].value = 66,
                5 => claims.get_mut(&1).unwrap().cells[0].known = false,
                _ => claims.get_mut(&1).unwrap().cells[0].version = 1,
            }
            assert_eq!(oracle.retained_guard(&claims), Err(Code::ResultMutated));
            assert_eq!(oracle.checks[Category::Ownership.index()].failed, 1);
            assert_eq!(oracle.immutable_results[&1], read_claim());
        }
    }

    fn completed_hit() -> Oracle<'static> {
        let mut oracle = Oracle::new(&[65], 4096, 1);
        let span = Span { start: 0, len: 1 };
        oracle.bind(0, 0, 0).unwrap();
        oracle.admit(
            RequestDescriptor {
                id: 1,
                span,
                kind: RequestKind::Fill,
                data: &[],
                fua: false,
                admitted_at: 1,
            },
            true,
        );
        for (id, target, source) in [
            (1, IoTarget::OriginRead, None),
            (2, IoTarget::CacheWrite, Some(1)),
        ] {
            oracle
                .forward(IoDescriptor {
                    id,
                    parent: 1,
                    target,
                    span,
                    slot: (id == 2).then_some(0),
                    source,
                })
                .unwrap();
            oracle.transfer(id, 0, 1, id as usize * 3);
            oracle
                .complete(
                    id,
                    Completion {
                        status: IoStatus::Ok,
                        reported: 1,
                    },
                )
                .unwrap();
        }
        oracle.publish(2).unwrap();
        oracle.admit(
            RequestDescriptor {
                id: 2,
                span,
                kind: RequestKind::Read,
                data: &[],
                fua: false,
                admitted_at: 9,
            },
            true,
        );
        oracle
            .forward(IoDescriptor {
                id: 3,
                parent: 2,
                target: IoTarget::CacheRead,
                span,
                slot: Some(0),
                source: None,
            })
            .unwrap();
        oracle.transfer(3, 0, 1, 11);
        oracle
            .complete(
                3,
                Completion {
                    status: IoStatus::Ok,
                    reported: 1,
                },
            )
            .unwrap();
        oracle
    }

    #[test]
    fn forced_cache_claim_is_rejected_after_mutation_rebind_or_epoch_without_policy_help() {
        for change in 0..3 {
            let mut oracle = completed_hit();
            match change {
                0 => oracle.admit(
                    RequestDescriptor {
                        id: 3,
                        span: Span { start: 0, len: 1 },
                        kind: RequestKind::Write,
                        data: &[65], // Byte equality does not rescue lineage.
                        fua: false,
                        admitted_at: 13,
                    },
                    true,
                ),
                1 => oracle.bind(0, 0, 13).unwrap(),
                _ => oracle.invalidate(13),
            }
            let claim = CallerResult {
                status: IoStatus::Ok,
                ..read_claim()
            };
            assert_eq!(oracle.commit(2, 3, &claim), Err(Code::StaleCacheResult));
            assert_eq!(oracle.checks[Category::CacheLineage.index()].passed, 2);
            assert_eq!(oracle.checks[Category::CacheLineage.index()].failed, 1);
            assert!(oracle.immutable_results.is_empty());
        }
    }

    #[test]
    fn durable_or_restore_claims_are_checked_against_independently_applied_history() {
        for persist in [true, false] {
            let mut oracle = Oracle::new(&[65], 4096, 1);
            let span = Span { start: 0, len: 1 };
            oracle.admit(
                RequestDescriptor {
                    id: 1,
                    span,
                    kind: RequestKind::Write,
                    data: &[66],
                    fua: false,
                    admitted_at: 0,
                },
                true,
            );
            oracle
                .forward(IoDescriptor {
                    id: 1,
                    parent: 1,
                    target: IoTarget::OriginWrite,
                    span,
                    slot: None,
                    source: None,
                })
                .unwrap();
            oracle.transfer(1, 0, 1, 2);
            let result = if persist {
                oracle.persist(1, 0, 1, &[DataCell::initial(65)])
            } else {
                oracle.power_loss(&[DataCell {
                    version: 3,
                    value: 66,
                    known: true,
                    covered: true,
                }])
            };
            assert_eq!(result, Err(Code::DurableHistoryMismatch));
            assert_eq!(oracle.checks[Category::Durability.index()].failed, 1);
        }
    }

    #[test]
    fn wrong_parent_delivery_is_rejected_without_candidate_validation() {
        let mut oracle = Oracle::new(&[65], 4096, 1);
        let span = Span { start: 0, len: 1 };
        for id in [1, 2] {
            oracle.admit(
                RequestDescriptor {
                    id,
                    span,
                    kind: RequestKind::Read,
                    data: &[],
                    fua: false,
                    admitted_at: id as usize - 1,
                },
                true,
            );
        }
        oracle
            .forward(IoDescriptor {
                id: 1,
                parent: 1,
                target: IoTarget::OriginRead,
                span,
                slot: None,
                source: None,
            })
            .unwrap();
        oracle.transfer(1, 0, 1, 3);
        oracle
            .complete(
                1,
                Completion {
                    status: IoStatus::Ok,
                    reported: 1,
                },
            )
            .unwrap();
        let wrong = CallerResult {
            status: IoStatus::Ok,
            reported: 1,
            cells: vec![DataCell::initial(65)],
        };
        assert_eq!(oracle.commit(2, 1, &wrong), Err(Code::InvalidOwnership));
        assert_eq!(oracle.checks[Category::OriginResults.index()].failed, 1);
        assert!(oracle.immutable_results.is_empty());
    }
}
