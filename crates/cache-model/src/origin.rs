//! A bounded byte oracle component. Discarded bytes are unknown, not zero.
use cache_types::range::{RangeError, checked_range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginError {
    CapacityTooLarge,
    Range(RangeError),
    UnknownRange,
}

#[derive(Debug)]
pub struct MemoryOrigin {
    bytes: Vec<u8>,
    known: Vec<bool>,
}

impl MemoryOrigin {
    pub fn new(initial: Vec<u8>) -> Result<Self, OriginError> {
        if initial.len() > 1_048_576 {
            return Err(OriginError::CapacityTooLarge);
        }
        Ok(Self {
            known: vec![true; initial.len()],
            bytes: initial,
        })
    }
    pub fn read(&self, offset: u64, length: u64) -> Result<Vec<u8>, OriginError> {
        let range = self.range(offset, length)?;
        if self.known[range.clone()].iter().any(|known| !known) {
            return Err(OriginError::UnknownRange);
        }
        Ok(self.bytes[range].to_vec())
    }
    pub fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), OriginError> {
        let range = self.range(offset, data.len() as u64)?;
        self.bytes[range.clone()].copy_from_slice(data);
        self.known[range].fill(true);
        Ok(())
    }
    pub fn discard(&mut self, offset: u64, length: u64) -> Result<(), OriginError> {
        let range = self.range(offset, length)?;
        self.known[range].fill(false);
        Ok(())
    }

    fn range(&self, offset: u64, length: u64) -> Result<std::ops::Range<usize>, OriginError> {
        let range =
            checked_range(offset, length, self.bytes.len() as u64).map_err(OriginError::Range)?;
        // Both ends are bounded by a Vec length <= 1 MiB on the supported hosts.
        Ok(range.start as usize..range.end as usize)
    }
}
