//! Reference policy. It cannot call oracle freshness or ownership helpers.
use super::{
    lower::{ActualIo, Lower},
    types::{
        CallerResult, Completion, IoDescriptor, MAX_LIVE, MAX_OWNED_CELLS, RequestDescriptor,
        RequestKind, Span,
    },
};
use cache_types::{
    evidence::{Fault, ViolationCode as Code},
    trace::{Decision, IoStatus, IoTarget, TraceV1},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct Token {
    epoch: u64,
    generation: u64,
    line: usize,
    incarnation: u64,
    owner: Option<u64>,
}
struct Line {
    generation: u64,
    writers: usize,
    cacheable: bool,
}
#[derive(Default)]
struct Slot {
    line: Option<usize>,
    incarnation: u64,
    retired: bool,
    visible: bool,
    owner: Option<u64>,
}
pub(crate) struct Parent<'a> {
    pub descriptor: RequestDescriptor<'a>,
    pub admitted: bool,
    pub retired: bool,
    pub cancelled: bool,
    reserved_child: bool,
    fill_token: Option<Token>,
}
pub(crate) struct Policy<'a> {
    pub parents: BTreeMap<u64, Parent<'a>>,
    pub results: BTreeMap<u64, CallerResult>,
    pub owned_cells: usize,
    pub peak_owned_cells: usize,
    reserved_children: usize,
    lines: Vec<Line>,
    slots: Vec<Slot>,
    tokens: BTreeMap<u64, Token>,
    published: BTreeSet<u64>,
    epoch: u64,
    exhausted: bool,
    active: bool,
    capacity: usize,
    line_size: usize,
    limit: u64,
    fault: Fault,
}
impl<'a> Policy<'a> {
    pub fn new(trace: &TraceV1, fault: Fault) -> Self {
        Self {
            parents: BTreeMap::new(),
            results: BTreeMap::new(),
            owned_cells: 0,
            peak_owned_cells: 0,
            reserved_children: 0,
            lines: (0..trace.initial().len().div_ceil(trace.line_size() as usize))
                .map(|_| Line {
                    generation: 1,
                    writers: 0,
                    cacheable: true,
                })
                .collect(),
            slots: (0..trace.slot_count()).map(|_| Slot::default()).collect(),
            tokens: BTreeMap::new(),
            published: BTreeSet::new(),
            epoch: 1,
            exhausted: false,
            active: true,
            capacity: trace.initial().len(),
            line_size: trace.line_size() as usize,
            limit: trace.counter_limit(),
            fault,
        }
    }
    fn reserve(&mut self, cells: usize) {
        self.owned_cells += cells;
        self.peak_owned_cells = self.peak_owned_cells.max(self.owned_cells);
    }
    fn intersecting(&self, span: Span) -> std::ops::Range<usize> {
        if span.len == 0 {
            0..0
        } else {
            span.start / self.line_size..(span.end() - 1) / self.line_size + 1
        }
    }
    fn hit_line(&self, span: Span) -> Option<usize> {
        if span.start >= self.capacity {
            return None;
        }
        let line = span.start / self.line_size;
        if span.len != 0 && (span.end() - 1) / self.line_size != line {
            None
        } else {
            Some(line)
        }
    }
    pub fn admit(&mut self, lower: &Lower<'_>, descriptor: RequestDescriptor<'a>) -> Decision {
        let mutation = matches!(descriptor.kind, RequestKind::Write | RequestKind::Discard);
        let cells = if matches!(descriptor.kind, RequestKind::Fill | RequestKind::Flush) {
            0
        } else {
            descriptor.span.len
        };
        let fill_line = descriptor.span.start / self.line_size;
        let eligible = descriptor.kind != RequestKind::Fill
            || (self.active
                && self.lines[fill_line].cacheable
                && self.lines[fill_line].writers == 0);
        let admitted = eligible
            && self
                .parents
                .values()
                .filter(|p| !p.retired && p.admitted)
                .count()
                < MAX_LIVE
            && cells <= MAX_OWNED_CELLS - self.owned_cells
            && (!mutation || lower.pending() as usize + self.reserved_children < MAX_LIVE);
        let fill_token = if admitted && descriptor.kind == RequestKind::Fill {
            Some(Token {
                epoch: self.epoch,
                generation: self.lines[fill_line].generation,
                line: fill_line,
                incarnation: 0,
                owner: None,
            })
        } else {
            None
        };
        if admitted {
            self.reserve(cells);
            if mutation {
                self.reserved_children += 1;
                let range = self.intersecting(descriptor.span);
                for line in range.clone() {
                    let state = &mut self.lines[line];
                    if state.generation == self.limit {
                        state.cacheable = false;
                    } else {
                        state.generation += 1;
                    }
                    state.writers += 1;
                }
                for slot in &mut self.slots {
                    if slot.line.is_some_and(|line| range.contains(&line)) {
                        slot.visible = false;
                    }
                }
            }
        }
        self.parents.insert(
            descriptor.id,
            Parent {
                descriptor,
                admitted,
                retired: false,
                cancelled: false,
                reserved_child: admitted && mutation,
                fill_token,
            },
        );
        if admitted {
            Decision::Accepted
        } else {
            Decision::Rejected
        }
    }
    pub fn descriptor(
        &self,
        lower: &Lower<'_>,
        id: u64,
        parent: u64,
        target: IoTarget,
        slot: Option<u64>,
        source: Option<u64>,
    ) -> Result<IoDescriptor, Code> {
        let p = self.parents.get(&parent).ok_or(Code::InvalidPhase)?;
        if !p.admitted
            || p.retired
            || self.results.contains_key(&parent)
            || lower
                .ios
                .values()
                .any(|io| io.descriptor.parent == parent && io.descriptor.target == target)
        {
            return Err(Code::InvalidPhase);
        }
        let correct = matches!(
            (p.descriptor.kind, target),
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
        if !correct {
            return Err(Code::InvalidPhase);
        }
        Ok(IoDescriptor {
            id,
            parent,
            target,
            span: p.descriptor.span,
            slot,
            source,
        })
    }
    fn slot_busy(&self, lower: &Lower<'_>, slot: usize, exclusive: bool) -> bool {
        lower.ios.values().any(|io| {
            io.descriptor.slot == Some(slot as u64)
                && io.completion.is_none()
                && (exclusive || io.descriptor.target == IoTarget::CacheWrite)
                && !(self.fault == Fault::LateSlotReuse
                    && (io.cancelled
                        || self.parents[&io.descriptor.parent].cancelled
                        || !self.active))
        })
    }
    fn full_success(io: &ActualIo<'_>) -> bool {
        io.completion
            == Some(Completion {
                status: IoStatus::Ok,
                reported: io.descriptor.span.len as u64,
            })
            && io.cells.len() == io.descriptor.span.len
            && io.cells.iter().all(|c| c.covered && c.known)
    }
    pub fn admit_io(&mut self, lower: &Lower<'_>, desc: IoDescriptor) -> Decision {
        let parent = &self.parents[&desc.parent];
        let mut token = None;
        if desc.target == IoTarget::OriginFlush
            && self.parents.values().any(|p| {
                p.admitted
                    && p.descriptor.admitted_at < parent.descriptor.admitted_at
                    && matches!(p.descriptor.kind, RequestKind::Write | RequestKind::Discard)
                    && !lower.ios.values().any(|io| {
                        io.descriptor.parent == p.descriptor.id && io.completion.is_some()
                    })
            })
        {
            return Decision::Pending;
        }
        if matches!(desc.target, IoTarget::CacheRead | IoTarget::CacheWrite) {
            let slot_id = desc.slot.expect("F05 validates cache slot") as usize;
            let slot = &self.slots[slot_id];
            let Some(line) = self.hit_line(desc.span) else {
                return Decision::Rejected;
            };
            if !self.active || slot.retired || slot.line != Some(line) {
                return Decision::Rejected;
            }
            if desc.target == IoTarget::CacheRead {
                if !slot.visible || !self.lines[line].cacheable || self.lines[line].writers != 0 {
                    return Decision::Rejected;
                }
                if self.slot_busy(lower, slot_id, false) {
                    return Decision::Pending;
                }
                token = Some(Token {
                    epoch: self.epoch,
                    generation: self.lines[line].generation,
                    line,
                    incarnation: slot.incarnation,
                    owner: slot.owner,
                });
            } else {
                let Some(source) = lower
                    .ios
                    .get(&desc.source.expect("F05 validates fill source"))
                else {
                    return Decision::Pending;
                };
                if source.completion.is_none() {
                    return Decision::Pending;
                }
                if !Self::full_success(source) {
                    return Decision::Rejected;
                }
                if self.slot_busy(lower, slot_id, true) {
                    return Decision::Pending;
                }
                // Deliberately no fill-token freshness check here: stale physical writes are modelled.
                token = parent.fill_token;
            }
        }
        let needed = if matches!(
            desc.target,
            IoTarget::OriginRead | IoTarget::CacheRead | IoTarget::CacheWrite
        ) {
            desc.span.len
        } else {
            0
        };
        if (!parent.reserved_child && lower.pending() as usize + self.reserved_children >= MAX_LIVE)
            || needed > MAX_OWNED_CELLS - self.owned_cells
        {
            return Decision::Pending;
        }
        if desc.target == IoTarget::CacheWrite {
            let slot = &mut self.slots[desc.slot.expect("cache slot") as usize];
            if slot.incarnation == self.limit {
                slot.retired = true;
                slot.visible = false;
                return Decision::Rejected;
            }
            slot.incarnation += 1;
            slot.visible = false;
            slot.owner = Some(desc.id);
            let captured = token
                .as_mut()
                .expect("admitted fill has a pre-sample token");
            captured.incarnation = slot.incarnation;
            captured.owner = Some(desc.id);
        }
        if let Some(t) = token {
            self.tokens.insert(desc.id, t);
        }
        self.reserve(needed);
        let parent = self.parents.get_mut(&desc.parent).expect("declared parent");
        if parent.reserved_child {
            parent.reserved_child = false;
            self.reserved_children -= 1;
        }
        Decision::Accepted
    }
    fn fresh(&self, token: Token, slot_id: usize, ignore_fill_freshness: bool) -> bool {
        let slot = &self.slots[slot_id];
        let line = &self.lines[token.line];
        self.active
            && !slot.retired
            && line.cacheable
            && line.writers == 0
            && slot.line == Some(token.line)
            && slot.incarnation == token.incarnation
            && slot.owner == token.owner
            && (ignore_fill_freshness
                || (self.epoch == token.epoch && line.generation == token.generation))
    }
    pub fn publish(&mut self, lower: &Lower<'_>, id: u64) -> Result<Decision, Code> {
        let io = lower.io(id)?;
        if io.descriptor.target != IoTarget::CacheWrite
            || self.parents[&io.descriptor.parent].retired
            || self.published.contains(&id)
        {
            return Err(Code::InvalidPhase);
        }
        if io.completion.is_none() {
            return Ok(Decision::Pending);
        }
        let slot = io.descriptor.slot.expect("cache slot") as usize;
        let source = lower.io(io.descriptor.source.expect("cache source"))?;
        let token = self.tokens[&id];
        if !Self::full_success(io)
            || !Self::full_success(source)
            || self.slot_busy(lower, slot, false)
            || !self.fresh(token, slot, self.fault == Fault::StaleFill)
        {
            return Ok(Decision::Rejected);
        }
        self.slots[slot].visible = true;
        self.published.insert(id);
        Ok(Decision::Accepted)
    }
    pub fn commit(&mut self, lower: &Lower<'_>, request: u64, id: u64) -> Result<Decision, Code> {
        if self.results.contains_key(&request)
            || self.parents.get(&request).is_none_or(|p| p.retired)
        {
            return Err(Code::InvalidPhase);
        }
        let io = lower.io(id)?;
        if io.descriptor.parent != request {
            return Err(Code::InvalidPhase);
        }
        let read = matches!(
            io.descriptor.target,
            IoTarget::OriginRead | IoTarget::CacheRead
        );
        let c = if let Some(c) = io.completion {
            c
        } else if !read && self.fault == Fault::EarlyAck {
            Completion {
                status: IoStatus::Ok,
                reported: io.descriptor.span.len as u64,
            }
        } else {
            return Ok(Decision::Pending);
        };
        if io.descriptor.target == IoTarget::CacheRead {
            let slot = io.descriptor.slot.expect("cache slot") as usize;
            if !Self::full_success(io)
                || !self.slots[slot].visible
                || !self.fresh(self.tokens[&id], slot, false)
            {
                return Ok(Decision::Rejected);
            }
        }
        let cells = if read {
            io.cells[..c.reported as usize].to_vec()
        } else {
            Vec::new()
        };
        if read {
            self.owned_cells -= io.descriptor.span.len - c.reported as usize;
        }
        self.results.insert(
            request,
            CallerResult {
                status: c.status,
                reported: c.reported,
                cells,
            },
        );
        Ok(Decision::Accepted)
    }
    pub fn complete(&mut self, lower: &Lower<'_>, id: u64) {
        let io = &lower.ios[&id];
        if matches!(
            io.descriptor.target,
            IoTarget::OriginWrite | IoTarget::OriginDiscard
        ) {
            for line in self.intersecting(io.descriptor.span) {
                self.lines[line].writers -= 1;
            }
        }
    }
    pub fn bind(&mut self, lower: &Lower<'_>, slot_id: usize, line: usize) -> Decision {
        let slot = &self.slots[slot_id];
        if slot.retired || slot.incarnation == self.limit {
            self.slots[slot_id].retired = true;
            self.slots[slot_id].visible = false;
            return Decision::Rejected;
        }
        if self.slot_busy(lower, slot_id, true) {
            return Decision::Pending;
        }
        let slot = &mut self.slots[slot_id];
        slot.incarnation += 1;
        slot.line = Some(line);
        slot.owner = None;
        slot.visible = false;
        Decision::Accepted
    }
    pub fn invalidate(&mut self) {
        self.active = false;
        if self.epoch == self.limit {
            self.exhausted = true;
        } else {
            self.epoch += 1;
        }
        for slot in &mut self.slots {
            slot.visible = false;
        }
    }
    pub fn resume(&mut self, lower: &Lower<'_>) -> Decision {
        if self.active {
            return Decision::Accepted;
        }
        if self.exhausted {
            return Decision::Rejected;
        }
        if lower.pending() != 0
            || self.reserved_children != 0
            || self.lines.iter().any(|l| l.writers != 0)
        {
            return Decision::Pending;
        }
        for slot in &mut self.slots {
            slot.visible = false;
        }
        self.active = true;
        Decision::Accepted
    }
    pub fn visible(&self, slot: usize) -> bool {
        self.slots[slot].visible
    }
    pub fn power_loss(&mut self) {
        self.active = false;
        for s in &mut self.slots {
            s.visible = false;
        }
    }
    pub fn cancel_parent(&mut self, request: u64) -> Result<(), Code> {
        let p = self.parents.get_mut(&request).ok_or(Code::InvalidPhase)?;
        if p.retired {
            return Err(Code::InvalidPhase);
        }
        p.cancelled = true;
        Ok(())
    }
    pub fn retire(&mut self, lower: &Lower<'_>, request: u64) -> Result<Decision, Code> {
        let p = self.parents.get(&request).ok_or(Code::InvalidPhase)?;
        if p.retired {
            return Err(Code::InvalidPhase);
        }
        if lower
            .ios
            .values()
            .any(|io| io.descriptor.parent == request && io.completion.is_none())
            || (p.admitted
                && p.descriptor.kind != RequestKind::Fill
                && !self.results.contains_key(&request))
        {
            return Ok(Decision::Pending);
        }
        for io in lower
            .ios
            .values()
            .filter(|io| io.descriptor.parent == request)
        {
            if matches!(
                io.descriptor.target,
                IoTarget::OriginRead | IoTarget::CacheRead | IoTarget::CacheWrite
            ) {
                self.owned_cells -= io.cells.len();
            }
        }
        self.parents
            .get_mut(&request)
            .expect("declared parent")
            .retired = true;
        Ok(Decision::Accepted)
    }
    pub fn pending(&self) -> u64 {
        self.parents
            .iter()
            .filter(|(id, p)| {
                p.admitted
                    && p.descriptor.kind != RequestKind::Fill
                    && !self.results.contains_key(id)
            })
            .count() as u64
    }
}
