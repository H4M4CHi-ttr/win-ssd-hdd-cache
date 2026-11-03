//! Original Rust host-only feasibility experiment. Not a production engine,
//! independent D07 oracle, Windows adapter, or full trace-v1 implementation.
#![forbid(unsafe_code)]

use std::mem::size_of;

const CAPACITY: usize = 8192;
const LINE: usize = 4096;
const MAX_RECORDS: usize = 64;
const MAX_CELLS: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    LateToken,
    BooleanWriter,
    FirstLineOnly,
    IgnoreProvenance,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    IoError,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denied {
    Bounds,
    State,
    Capacity,
    Rejected,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerResult {
    pub status: Status,
    pub reported: usize,
    pub data: Vec<Option<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Memory {
    pub record_count: usize,
    pub reserved_cells: usize,
    pub record_vec_capacity_bytes: usize,
    pub owned_vec_capacity_bytes: usize,
    pub fake_media_capacity_bytes: usize,
    pub fixed_policy_bytes: usize,
}

#[derive(Clone, Copy)]
enum Kind {
    Write,
    Discard,
    Read,
    Fill { line: usize },
    CacheWrite { slot: usize },
    Hit { slot: usize },
}
impl Kind {
    fn is_caller_read(self) -> bool {
        matches!(self, Self::Read | Self::Hit { .. })
    }
}
#[derive(Clone, Copy)]
struct Token {
    epoch: u64,
    generation: u64,
    incarnation: u64,
}
struct Record {
    kind: Kind,
    offset: usize,
    length: usize,
    token: Option<Token>,
    data: Vec<Option<u8>>,
    transferred: Vec<bool>,
    completion: Option<(Status, usize)>,
    result: Option<CallerResult>,
    published: bool,
    cancelled: bool,
}
#[derive(Clone, Copy)]
struct LineState {
    generation: u64,
    writers: u32,
    retired: bool,
}
#[derive(Clone, Copy)]
struct Slot {
    line: Option<usize>,
    incarnation: u64,
    visible: bool,
    retired: bool,
    owner: Option<usize>,
    writer: Option<usize>,
    readers: u32,
}

/// Closed fake storage. Tests choose when a fragment transfers, but never supply
/// successful read bytes or a ready-made completion coverage boolean.
struct FakeMedia {
    origin: Vec<Option<u8>>,
    slots: [Vec<Option<u8>>; 2],
}

pub struct Experiment {
    fault: Fault,
    epoch: u64,
    limit: u64,
    active: bool,
    lines: [LineState; 2],
    slots: [Slot; 2],
    records: Vec<Record>,
    reserved: usize,
    media: FakeMedia,
}

impl Experiment {
    pub fn new(fault: Fault) -> Self {
        Self::with_limit(fault, u64::MAX)
    }
    pub fn with_limit(fault: Fault, limit: u64) -> Self {
        assert!(limit > 0, "experiment counter limit must be positive");
        Self {
            fault,
            epoch: 1,
            limit,
            active: true,
            lines: [LineState {
                generation: 1,
                writers: 0,
                retired: false,
            }; 2],
            slots: [Slot {
                line: None,
                incarnation: 0,
                visible: false,
                retired: false,
                owner: None,
                writer: None,
                readers: 0,
            }; 2],
            records: Vec::with_capacity(MAX_RECORDS),
            reserved: 0,
            media: FakeMedia {
                origin: vec![Some(0); CAPACITY],
                slots: std::array::from_fn(|_| vec![None; LINE]),
            },
        }
    }

    fn range(offset: u64, length: usize, capacity: usize) -> Result<usize, Denied> {
        if offset > capacity as u64 || length as u64 > capacity as u64 - offset {
            return Err(Denied::Bounds);
        }
        Ok(offset as usize)
    }
    fn line_range(offset: usize, length: usize) -> std::ops::Range<usize> {
        let first = offset / LINE;
        if length == 0 {
            first..first
        } else {
            first..(offset + length - 1) / LINE + 1
        }
    }
    fn check_capacity(&self, kind: Kind, length: usize) -> Result<usize, Denied> {
        let cells = length
            .checked_mul(if kind.is_caller_read() { 2 } else { 1 })
            .ok_or(Denied::Capacity)?;
        if self.records.len() == MAX_RECORDS || cells > MAX_CELLS - self.reserved {
            return Err(Denied::Capacity);
        }
        Ok(cells)
    }
    fn append(
        &mut self,
        kind: Kind,
        offset: usize,
        data: Vec<Option<u8>>,
        token: Option<Token>,
        reserved: usize,
    ) -> usize {
        let id = self.records.len();
        let length = data.len();
        self.records.push(Record {
            kind,
            offset,
            length,
            token,
            data,
            transferred: vec![false; length],
            completion: None,
            result: None,
            published: false,
            cancelled: false,
        });
        self.reserved += reserved;
        id
    }

    fn mutation(
        &mut self,
        offset: u64,
        data: &[u8],
        discard_length: Option<usize>,
    ) -> Result<usize, Denied> {
        let kind = if discard_length.is_some() {
            Kind::Discard
        } else {
            Kind::Write
        };
        let length = discard_length.unwrap_or(data.len());
        let offset = Self::range(offset, length, CAPACITY)?;
        let cells = self.check_capacity(kind, length)?;
        let bytes = if discard_length.is_some() {
            vec![None; length]
        } else {
            data.iter().copied().map(Some).collect()
        };
        let id = self.append(kind, offset, bytes, None, cells);
        let first = offset / LINE;
        for line in Self::line_range(offset, length) {
            let state = &mut self.lines[line];
            state.writers = if self.fault == Fault::BooleanWriter {
                1
            } else {
                state.writers + 1
            };
            if self.fault == Fault::FirstLineOnly && line != first {
                continue;
            }
            if state.generation == self.limit {
                state.retired = true;
            } else {
                state.generation += 1;
            }
            for slot in &mut self.slots {
                if slot.line == Some(line) {
                    slot.visible = false;
                }
            }
        }
        Ok(id)
    }
    pub fn write(&mut self, offset: u64, data: &[u8]) -> Result<usize, Denied> {
        self.mutation(offset, data, None)
    }
    pub fn discard(&mut self, offset: u64, length: usize) -> Result<usize, Denied> {
        self.mutation(offset, &[], Some(length))
    }
    pub fn read(&mut self, offset: u64, length: usize) -> Result<usize, Denied> {
        let offset = Self::range(offset, length, CAPACITY)?;
        let cells = self.check_capacity(Kind::Read, length)?;
        Ok(self.append(Kind::Read, offset, vec![None; length], None, cells))
    }
    pub fn fill(&mut self, line: usize) -> Result<usize, Denied> {
        let state = self.lines.get(line).ok_or(Denied::Bounds)?;
        if !self.active || state.retired || state.writers != 0 {
            return Err(Denied::Rejected);
        }
        let token = Token {
            epoch: self.epoch,
            generation: state.generation,
            incarnation: 0,
        };
        let cells = self.check_capacity(Kind::Fill { line }, LINE)?;
        Ok(self.append(
            Kind::Fill { line },
            line * LINE,
            vec![None; LINE],
            Some(token),
            cells,
        ))
    }
    pub fn bind(&mut self, slot: usize, line: usize) -> Result<(), Denied> {
        if line >= self.lines.len() {
            return Err(Denied::Bounds);
        }
        let s = self.slots.get_mut(slot).ok_or(Denied::Bounds)?;
        if s.retired {
            return Err(Denied::Rejected);
        }
        if s.writer.is_some() || s.readers != 0 {
            return Err(Denied::Pending);
        }
        if s.incarnation == self.limit {
            s.retired = true;
            s.visible = false;
            return Err(Denied::Rejected);
        }
        s.incarnation += 1;
        s.line = Some(line);
        s.visible = false;
        s.owner = None;
        Ok(())
    }
    pub fn cache_write(&mut self, source: usize, slot: usize) -> Result<usize, Denied> {
        let r = self.records.get(source).ok_or(Denied::Bounds)?;
        let Kind::Fill { line } = r.kind else {
            return Err(Denied::State);
        };
        let s = self.slots.get(slot).ok_or(Denied::Bounds)?;
        if !self.active || s.retired || s.line != Some(line) {
            return Err(Denied::Rejected);
        }
        if r.completion.is_none() {
            return Err(Denied::Pending);
        }
        if r.completion != Some((Status::Ok, LINE))
            || !r.transferred.iter().all(|b| *b)
            || r.data.iter().any(Option::is_none)
        {
            return Err(Denied::Rejected);
        }
        if s.writer.is_some() || s.readers != 0 {
            return Err(Denied::Pending);
        }
        if s.incarnation == self.limit {
            self.slots[slot].retired = true;
            self.slots[slot].visible = false;
            return Err(Denied::Rejected);
        }
        let cells = self.check_capacity(Kind::CacheWrite { slot }, LINE)?;
        let token = Token {
            incarnation: s.incarnation + 1,
            ..r.token.ok_or(Denied::State)?
        };
        let data = r.data.clone();
        let id = self.append(
            Kind::CacheWrite { slot },
            line * LINE,
            data,
            Some(token),
            cells,
        );
        let s = &mut self.slots[slot];
        s.incarnation = token.incarnation;
        s.owner = Some(id);
        s.writer = Some(id);
        if self.fault != Fault::IgnoreProvenance {
            s.visible = false;
        }
        Ok(id)
    }
    pub fn hit(&mut self, offset: u64, length: usize, slot: usize) -> Result<usize, Denied> {
        let offset = Self::range(offset, length, CAPACITY)?;
        let s = self.slots.get(slot).ok_or(Denied::Bounds)?;
        let line = offset / LINE;
        if line >= self.lines.len()
            || length > LINE - offset % LINE
            || !self.active
            || !s.visible
            || s.retired
            || s.line != Some(line)
            || self.lines[line].writers != 0
            || self.lines[line].retired
        {
            return Err(Denied::Rejected);
        }
        if s.writer.is_some() {
            return Err(Denied::Pending);
        }
        let cells = self.check_capacity(Kind::Hit { slot }, length)?;
        let token = Token {
            epoch: self.epoch,
            generation: self.lines[line].generation,
            incarnation: s.incarnation,
        };
        let id = self.append(
            Kind::Hit { slot },
            offset,
            vec![None; length],
            Some(token),
            cells,
        );
        self.slots[slot].readers += 1;
        Ok(id)
    }

    pub fn transfer(&mut self, id: usize, offset: usize, length: usize) -> Result<(), Denied> {
        let r = self.records.get_mut(id).ok_or(Denied::Bounds)?;
        Self::range(offset as u64, length, r.length)?;
        if r.completion.is_some() || r.transferred[offset..offset + length].iter().any(|b| *b) {
            return Err(Denied::State);
        }
        for i in offset..offset + length {
            match r.kind {
                Kind::Write | Kind::Discard => self.media.origin[r.offset + i] = r.data[i],
                Kind::Read | Kind::Fill { .. } => r.data[i] = self.media.origin[r.offset + i],
                Kind::CacheWrite { slot } => self.media.slots[slot][i] = r.data[i],
                Kind::Hit { slot } => r.data[i] = self.media.slots[slot][r.offset % LINE + i],
            }
            r.transferred[i] = true;
        }
        Ok(())
    }
    pub fn complete(&mut self, id: usize, status: Status, reported: usize) -> Result<(), Denied> {
        let r = self.records.get_mut(id).ok_or(Denied::Bounds)?;
        if r.completion.is_some() {
            return Err(Denied::State);
        }
        if reported > r.length {
            return Err(Denied::Bounds);
        }
        if matches!(
            r.kind,
            Kind::Write | Kind::Discard | Kind::Read | Kind::Fill { .. }
        ) && !r.transferred[..reported].iter().all(|b| *b)
        {
            return Err(Denied::State);
        }
        r.completion = Some((status, reported));
        match r.kind {
            Kind::Write | Kind::Discard => {
                for line in Self::line_range(r.offset, r.length) {
                    self.lines[line].writers = if self.fault == Fault::BooleanWriter {
                        0
                    } else {
                        self.lines[line].writers - 1
                    };
                }
            }
            Kind::Fill { line } if self.fault == Fault::LateToken => {
                r.token = Some(Token {
                    epoch: self.epoch,
                    generation: self.lines[line].generation,
                    incarnation: 0,
                })
            }
            Kind::CacheWrite { slot } => self.slots[slot].writer = None,
            Kind::Hit { slot } => self.slots[slot].readers -= 1,
            _ => {}
        }
        Ok(())
    }
    fn full_success(r: &Record) -> bool {
        r.completion == Some((Status::Ok, r.length))
            && r.transferred.iter().all(|b| *b)
            && r.data.iter().all(Option::is_some)
    }
    fn fresh(&self, r: &Record, slot: usize) -> bool {
        let Some(t) = r.token else {
            return false;
        };
        let line = r.offset / LINE;
        let l = self.lines[line];
        self.active
            && !l.retired
            && l.writers == 0
            && t.epoch == self.epoch
            && t.generation == l.generation
            && self.slots[slot].line == Some(line)
    }
    pub fn publish(&mut self, id: usize) -> Result<(), Denied> {
        let r = self.records.get(id).ok_or(Denied::Bounds)?;
        let Kind::CacheWrite { slot } = r.kind else {
            return Err(Denied::State);
        };
        if r.published {
            return Err(Denied::State);
        }
        if r.completion.is_none() {
            return Err(Denied::Pending);
        }
        let s = self.slots[slot];
        if !Self::full_success(r)
            || !self.fresh(r, slot)
            || s.retired
            || s.writer.is_some()
            || (self.fault != Fault::IgnoreProvenance
                && (s.owner != Some(id)
                    || r.token.ok_or(Denied::State)?.incarnation != s.incarnation))
        {
            return Err(Denied::Rejected);
        }
        self.slots[slot].visible = true;
        self.records[id].published = true;
        Ok(())
    }
    pub fn commit(&mut self, id: usize) -> Result<&CallerResult, Denied> {
        let r = self.records.get(id).ok_or(Denied::Bounds)?;
        if r.result.is_some() || matches!(r.kind, Kind::Fill { .. } | Kind::CacheWrite { .. }) {
            return Err(Denied::State);
        }
        let (status, reported) = r.completion.ok_or(Denied::Pending)?;
        if let Kind::Hit { slot } = r.kind
            && (!Self::full_success(r)
                || !self.fresh(r, slot)
                || !self.slots[slot].visible
                || r.token.ok_or(Denied::State)?.incarnation != self.slots[slot].incarnation)
        {
            return Err(Denied::Rejected);
        }
        let data = if r.kind.is_caller_read() {
            r.data[..reported].to_vec()
        } else {
            Vec::new()
        };
        let result = CallerResult {
            status,
            reported,
            data,
        };
        self.records[id].result = Some(result);
        self.records[id].result.as_ref().ok_or(Denied::State)
    }
    pub fn cancel(&mut self, id: usize) -> Result<(), Denied> {
        self.records.get_mut(id).ok_or(Denied::Bounds)?.cancelled = true;
        Ok(())
    }
    pub fn invalidate_epoch(&mut self) {
        self.active = false;
        if self.epoch < self.limit {
            self.epoch += 1;
        }
        for s in &mut self.slots {
            s.visible = false;
        }
    }
    pub fn visible(&self, slot: usize) -> Result<bool, Denied> {
        self.slots
            .get(slot)
            .map(|s| s.visible)
            .ok_or(Denied::Bounds)
    }
    pub fn committed(&self, id: usize) -> Result<Option<&CallerResult>, Denied> {
        self.records
            .get(id)
            .map(|r| r.result.as_ref())
            .ok_or(Denied::Bounds)
    }
    pub fn origin(&self, offset: u64, length: usize) -> Result<&[Option<u8>], Denied> {
        let offset = Self::range(offset, length, CAPACITY)?;
        Ok(&self.media.origin[offset..offset + length])
    }
    pub fn memory(&self) -> Memory {
        Memory {
            record_count: self.records.len(),
            reserved_cells: self.reserved,
            record_vec_capacity_bytes: self.records.capacity() * size_of::<Record>(),
            owned_vec_capacity_bytes: self
                .records
                .iter()
                .map(|r| {
                    r.data.capacity() * size_of::<Option<u8>>()
                        + r.transferred.capacity() * size_of::<bool>()
                        + r.result
                            .as_ref()
                            .map_or(0, |v| v.data.capacity() * size_of::<Option<u8>>())
                })
                .sum(),
            fake_media_capacity_bytes: (self.media.origin.capacity()
                + self.media.slots.iter().map(Vec::capacity).sum::<usize>())
                * size_of::<Option<u8>>(),
            fixed_policy_bytes: size_of::<Self>(),
        }
    }
}
