//! Actual fake media and actual child transfers. Never invokes candidate policy.
use super::types::{Completion, DataCell, IoDescriptor, RequestDescriptor};
use cache_types::{evidence::ViolationCode as Code, trace::IoTarget};
use std::collections::BTreeMap;

pub(crate) struct ActualIo<'a> {
    pub descriptor: IoDescriptor,
    pub cells: Vec<DataCell>,
    pub completion: Option<Completion>,
    data: &'a [u8],
    pub cancelled: bool,
}
pub(crate) struct Lower<'a> {
    pub origin: Vec<DataCell>,
    pub durable: Vec<DataCell>,
    pub ios: BTreeMap<u64, ActualIo<'a>>,
    pub slots: Vec<Vec<DataCell>>,
    line_size: usize,
}
impl<'a> Lower<'a> {
    pub fn new(initial: &[u8], line_size: usize, slots: usize) -> Self {
        Self {
            origin: initial.iter().copied().map(DataCell::initial).collect(),
            durable: initial.iter().copied().map(DataCell::initial).collect(),
            ios: BTreeMap::new(),
            slots: (0..slots)
                .map(|_| vec![DataCell::default(); line_size])
                .collect(),
            line_size,
        }
    }
    pub fn forward(&mut self, descriptor: IoDescriptor, parent: RequestDescriptor<'a>) {
        self.ios.insert(
            descriptor.id,
            ActualIo {
                descriptor,
                cells: vec![DataCell::default(); descriptor.span.len],
                completion: None,
                data: parent.data,
                cancelled: false,
            },
        );
    }
    pub fn io(&self, id: u64) -> Result<&ActualIo<'a>, Code> {
        self.ios.get(&id).ok_or(Code::UnacceptedIo)
    }
    pub fn transfer(
        &mut self,
        id: u64,
        offset: usize,
        len: usize,
        event: usize,
    ) -> Result<(), Code> {
        let io = self.io(id)?;
        if io.completion.is_some() {
            return Err(Code::TransferAfterCompletion);
        }
        if io.cells[offset..offset + len].iter().any(|c| c.covered) {
            return Err(Code::TransferOverlap);
        }
        // Move the record within this atomic host step to borrow its immutable source
        // without an unaccounted temporary data-buffer clone.
        let mut io = self.ios.remove(&id).expect("checked accepted IO");
        for relative in offset..offset + len {
            let absolute = io.descriptor.span.start + relative;
            let cell = match io.descriptor.target {
                IoTarget::OriginRead => self.origin[absolute],
                IoTarget::OriginWrite => DataCell {
                    version: event as u64 + 1,
                    value: io.data[relative],
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
                    self.ios[&io.descriptor.source.expect("validated cache source")].cells[relative]
                }
                IoTarget::CacheRead => {
                    self.slots[io.descriptor.slot.expect("validated cache slot") as usize]
                        [absolute % self.line_size]
                }
                IoTarget::OriginFlush => unreachable!("F05 rejects flush transfer"),
            };
            match io.descriptor.target {
                IoTarget::OriginWrite | IoTarget::OriginDiscard => self.origin[absolute] = cell,
                IoTarget::CacheWrite => {
                    self.slots[io.descriptor.slot.expect("cache slot") as usize][relative] = cell
                }
                _ => {}
            }
            io.cells[relative] = cell;
        }
        self.ios.insert(id, io);
        Ok(())
    }
    pub fn complete(&mut self, id: u64, completion: Completion) -> Result<(), Code> {
        let io = self.ios.get_mut(&id).ok_or(Code::UnacceptedIo)?;
        if io.completion.is_some() {
            return Err(Code::DuplicateCompletion);
        }
        io.completion = Some(completion);
        Ok(())
    }
    pub fn persist(&mut self, id: u64, offset: usize, len: usize) -> Result<(), Code> {
        let io = self.io(id)?;
        if !matches!(
            io.descriptor.target,
            IoTarget::OriginWrite | IoTarget::OriginDiscard
        ) || !io.cells[offset..offset + len].iter().all(|c| c.covered)
        {
            return Err(Code::InvalidPhase);
        }
        let io = &self.ios[&id];
        for relative in offset..offset + len {
            let cell = io.cells[relative];
            let durable = &mut self.durable[io.descriptor.span.start + relative];
            if cell.version > durable.version {
                *durable = cell;
            }
        }
        Ok(())
    }
    pub fn retire(&mut self, parent: u64) {
        for io in self
            .ios
            .values_mut()
            .filter(|io| io.descriptor.parent == parent)
        {
            if matches!(
                io.descriptor.target,
                IoTarget::OriginRead | IoTarget::CacheRead | IoTarget::CacheWrite
            ) {
                io.cells = Vec::new();
            }
        }
    }
    pub fn pending(&self) -> u64 {
        self.ios
            .values()
            .filter(|io| io.completion.is_none())
            .count() as u64
    }
    pub fn power_loss(&mut self) {
        self.origin.copy_from_slice(&self.durable);
        for slot in &mut self.slots {
            slot.fill(DataCell::default());
        }
    }
}
