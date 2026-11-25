//! A two-slot read cache over two ordinary files, used by the integration test only.
use cache_types::range::{LineSize, checked_range, line_span};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

const BLOCK_BYTES: usize = 4096;
const SLOTS: usize = 2;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ReadFrom {
    Origin,
    Cache,
}

#[derive(Debug, Eq, PartialEq)]
struct BlockRead {
    bytes: Vec<u8>,
    from: ReadFrom,
}

#[derive(Debug)]
enum SectorOp<'a> {
    Read,
    Write(&'a [u8]),
    Flush,
    Unmap,
}

#[derive(Debug, Eq, PartialEq)]
enum SectorReply {
    Data(Vec<u8>),
    Written,
    Flushed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Publication {
    Published,
    Stale,
}

#[derive(Default, Debug, Clone, Copy)]
struct Counts {
    origin_reads: u64,
    origin_syncs: u64,
    cache_reads: u64,
    cache_read_errors: u64,
    cache_write_errors: u64,
}

struct Fill {
    block: u64,
    bytes: Vec<u8>,
    publishable: bool,
}

struct FileCache {
    origin: File,
    cache: File,
    capacity: u64,
    visible: [Option<u64>; SLOTS],
    pending: Option<Fill>,
    counts: Counts,
}

fn slot(block: u64) -> usize {
    (block % SLOTS as u64) as usize
}

fn slot_offset(slot: usize) -> u64 {
    (slot * BLOCK_BYTES) as u64
}

fn read_at(file: &mut File, offset: u64, length: usize) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; length];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn invalid_range() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "range outside origin file")
}

impl FileCache {
    fn new(origin: File, cache: File) -> io::Result<Self> {
        let original = origin.metadata()?;
        if !original.is_file() || !cache.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected ordinary files",
            ));
        }
        Ok(Self {
            origin,
            cache,
            capacity: original.len(),
            visible: [None; SLOTS],
            pending: None,
            counts: Counts::default(),
        })
    }

    fn sector_io(
        &mut self,
        first: u64,
        count: u32,
        force_unit_access: bool,
        op: SectorOp<'_>,
    ) -> io::Result<SectorReply> {
        self.sector_io_with_sync(first, count, force_unit_access, op, File::sync_all)
    }

    // Each request holds an exclusive borrow until its origin I/O and any barrier finish.
    fn sector_io_with_sync(
        &mut self,
        first: u64,
        count: u32,
        force_unit_access: bool,
        op: SectorOp<'_>,
        sync_origin: impl FnOnce(&File) -> io::Result<()>,
    ) -> io::Result<SectorReply> {
        self.sector_io_with_origin(
            first,
            count,
            force_unit_access,
            op,
            File::write_all,
            sync_origin,
        )
    }

    fn sector_io_with_origin(
        &mut self,
        first: u64,
        count: u32,
        force_unit_access: bool,
        op: SectorOp<'_>,
        write_origin: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
        sync_origin: impl FnOnce(&File) -> io::Result<()>,
    ) -> io::Result<SectorReply> {
        const SECTOR_BYTES: u64 = 512;
        const MAX_SECTORS: u32 = 128;

        if matches!(op, SectorOp::Unmap) {
            return Err(io::ErrorKind::Unsupported.into());
        }
        if self.capacity == 0 || !self.capacity.is_multiple_of(SECTOR_BYTES) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid sector geometry",
            ));
        }
        let offset = first.checked_mul(SECTOR_BYTES).ok_or_else(invalid_range)?;
        let length = u64::from(count) * SECTOR_BYTES;
        let range = checked_range(offset, length, self.capacity).map_err(|_| invalid_range())?;

        match op {
            SectorOp::Flush => {
                // A zero count means through end of device. Flushing the whole origin
                // covers both the explicit range and every earlier accepted write.
                if force_unit_access || range.start == self.capacity {
                    return Err(invalid_range());
                }
                self.counts.origin_syncs += 1;
                sync_origin(&self.origin)?;
                return Ok(SectorReply::Flushed);
            }
            SectorOp::Read | SectorOp::Write(_) if !(1..=MAX_SECTORS).contains(&count) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid sector count",
                ));
            }
            SectorOp::Write(bytes) => {
                if bytes.len() != length as usize {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "write payload length mismatch",
                    ));
                }
                self.write_at_with(range.start, bytes, write_origin)?;
                if force_unit_access {
                    self.counts.origin_syncs += 1;
                    sync_origin(&self.origin)?;
                }
                return Ok(SectorReply::Written);
            }
            SectorOp::Read if force_unit_access => {
                self.counts.origin_syncs += 1;
                sync_origin(&self.origin)?;
                self.counts.origin_reads += 1;
                return read_at(&mut self.origin, range.start, length as usize)
                    .map(SectorReply::Data);
            }
            SectorOp::Read => {}
            SectorOp::Unmap => return Err(io::ErrorKind::Unsupported.into()),
        }
        let mut output = Vec::with_capacity(length as usize);
        let mut pos = range.start;
        while pos < range.end {
            let within = (pos % BLOCK_BYTES as u64) as usize;
            let take = (range.end - pos).min((BLOCK_BYTES - within) as u64) as usize;
            let line = self.read_block(pos / BLOCK_BYTES as u64)?;
            let part = line
                .bytes
                .get(within..within + take)
                .ok_or(io::ErrorKind::UnexpectedEof)?;
            output.extend_from_slice(part);
            pos += take as u64;
        }
        Ok(SectorReply::Data(output))
    }

    fn block_range(&self, block: u64) -> io::Result<(u64, usize)> {
        let offset = block
            .checked_mul(BLOCK_BYTES as u64)
            .filter(|&offset| offset < self.capacity)
            .ok_or_else(invalid_range)?;
        let length = (self.capacity - offset).min(BLOCK_BYTES as u64);
        Ok((offset, length as usize))
    }

    fn sample(&mut self, block: u64, offset: u64, length: usize) -> io::Result<Fill> {
        self.counts.origin_reads += 1;
        let bytes = read_at(&mut self.origin, offset, length)?;
        Ok(Fill {
            block,
            bytes,
            publishable: true,
        })
    }

    fn store(&mut self, fill: &Fill) -> io::Result<Publication> {
        let slot = slot(fill.block);
        // A failed or partial physical write makes the former contents unusable.
        self.visible[slot] = None;
        let write = self
            .cache
            .seek(SeekFrom::Start(slot_offset(slot)))
            .and_then(|_| self.cache.write_all(&fill.bytes));
        if let Err(e) = write {
            self.counts.cache_write_errors += 1;
            return Err(e);
        }
        if fill.publishable {
            self.visible[slot] = Some(fill.block);
            Ok(Publication::Published)
        } else {
            Ok(Publication::Stale)
        }
    }

    fn read_block(&mut self, block: u64) -> io::Result<BlockRead> {
        let (offset, length) = self.block_range(block)?;
        let slot = slot(block);
        if self.visible[slot] == Some(block) {
            self.counts.cache_reads += 1;
            match read_at(&mut self.cache, slot_offset(slot), length) {
                Ok(bytes) => {
                    return Ok(BlockRead {
                        bytes,
                        from: ReadFrom::Cache,
                    });
                }
                Err(_) => {
                    self.counts.cache_read_errors += 1;
                    self.visible[slot] = None;
                }
            }
        }
        let fill = self.sample(block, offset, length)?;
        if self.pending.is_none() {
            // The origin read has already succeeded. Cache storage is best effort.
            let _ = self.store(&fill);
        }
        Ok(BlockRead {
            bytes: fill.bytes,
            from: ReadFrom::Origin,
        })
    }

    // Splitting capture and completion gives the test a deterministic delayed write.
    fn begin_fill(&mut self, block: u64) -> io::Result<()> {
        if self.pending.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "another fill is pending",
            ));
        }
        let (offset, length) = self.block_range(block)?;
        let fill = self.sample(block, offset, length)?;
        self.visible[slot(block)] = None;
        self.pending = Some(fill);
        Ok(())
    }

    fn complete_fill(&mut self) -> io::Result<Publication> {
        let fill = self
            .pending
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no pending fill"))?;
        // This exclusive borrow holds the slot through the physical write and publication.
        self.store(&fill)
    }

    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        self.write_at_with(offset, bytes, File::write_all)
    }

    fn write_at_with(
        &mut self,
        offset: u64,
        bytes: &[u8],
        write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let span = line_span(offset, bytes.len() as u64, self.capacity, LineSize::K4)
            .map_err(|_| invalid_range())?;
        if span.count == 0 {
            return Ok(());
        }
        let blocks = span.first..span.first + span.count;
        for tag in &mut self.visible {
            if tag.is_some_and(|block| blocks.contains(&block)) {
                *tag = None;
            }
        }
        if let Some(fill) = &mut self.pending
            && blocks.contains(&fill.block)
        {
            fill.publishable = false;
        }
        // Invalidation precedes any fallible origin I/O, including partial writes.
        self.origin.seek(SeekFrom::Start(offset))?;
        write(&mut self.origin, bytes)
    }
}

#[cfg(test)]
#[path = "checks.rs"]
mod tests;
