//! Fixed little-endian scalar protocol. No native structure is a wire image.
use crate::ProbeError;
use cache_capture_core::{Completion, Submission, SubmitSnapshot, decode_submit};
use std::num::NonZeroU64;

pub(crate) const HEADER_BYTES: usize = 144;
pub(crate) const RECORD_BYTES: usize = 96;
pub(crate) const REQUEST_BYTES: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    Begin = 1,
    Status = 2,
    Drain = 3,
    Stop = 4,
}

impl Command {
    pub const fn ioctl(self) -> u32 {
        0x8337_e000 + ((self as u32 - 1) * 4)
    }

    pub(crate) fn from_tag(tag: u32) -> Result<Self, ProbeError> {
        match tag {
            1 => Ok(Self::Begin),
            2 => Ok(Self::Status),
            3 => Ok(Self::Drain),
            4 => Ok(Self::Stop),
            _ => Err(ProbeError::InvalidArchive),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    command: Command,
    session: NonZeroU64,
    call: NonZeroU64,
    maximum: u32,
}

impl Request {
    pub fn new(
        command: Command,
        session: NonZeroU64,
        call: NonZeroU64,
        maximum: u32,
    ) -> Result<Self, ProbeError> {
        check_maximum(command, maximum)?;
        if command == Command::Begin && call.get() != 1 {
            return Err(ProbeError::InvalidArchive);
        }
        Ok(Self {
            command,
            session,
            call,
            maximum,
        })
    }

    pub fn to_bytes(self) -> [u8; REQUEST_BYTES] {
        let mut bytes = [0; REQUEST_BYTES];
        bytes[..8].copy_from_slice(b"WCHP04RQ");
        put32(&mut bytes, 8, 1);
        put32(&mut bytes, 12, REQUEST_BYTES as u32);
        put64(&mut bytes, 16, self.session.get());
        put64(&mut bytes, 24, 1);
        put64(&mut bytes, 32, self.call.get());
        put32(&mut bytes, 40, self.maximum);
        bytes
    }

    pub const fn command(self) -> Command {
        self.command
    }
    pub const fn session_id(self) -> u64 {
        self.session.get()
    }
    pub const fn call_id(self) -> u64 {
        self.call.get()
    }
    pub const fn maximum(self) -> u32 {
        self.maximum
    }
    pub const fn ioctl(self) -> u32 {
        self.command.ioctl()
    }
    pub const fn output_capacity(self) -> usize {
        if matches!(self.command, Command::Drain) {
            1680
        } else {
            144
        }
    }
}

fn check_maximum(command: Command, maximum: u32) -> Result<(), ProbeError> {
    if (command == Command::Drain && !(1..=16).contains(&maximum))
        || (command != Command::Drain && maximum != 0)
    {
        Err(ProbeError::InvalidArchive)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    Submit(Submission),
    Complete(Completion),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub(crate) seq: u64,
    pub(crate) request_id: u64,
    pub(crate) observation: Observation,
}

impl Record {
    pub const fn seq(&self) -> u64 {
        self.seq
    }
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }
    pub const fn observation(&self) -> Observation {
        self.observation
    }
    pub const fn submission(&self) -> Option<Submission> {
        match self.observation {
            Observation::Submit(s) => Some(s),
            _ => None,
        }
    }
    pub const fn status(&self) -> Option<u32> {
        match self.observation {
            Observation::Complete(c) => Some(c.status),
            _ => None,
        }
    }
    pub const fn information(&self) -> Option<u64> {
        match self.observation {
            Observation::Complete(c) => c.information,
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub(crate) command: Command,
    pub(crate) session: u64,
    pub(crate) call: u64,
    pub(crate) maximum: u32,
    pub(crate) state: u32,
    pub(crate) pnp: u32,
    pub(crate) active: u32,
    pub(crate) queued: u32,
    pub(crate) known: u32,
    pub(crate) faults: u32,
    pub(crate) attempted: Option<u64>,
    pub(crate) dropped: Option<u64>,
    pub(crate) popped: Option<u64>,
    pub(crate) skipped: Option<u64>,
    pub(crate) records: Vec<Record>,
}

impl Frame {
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    pub const fn command(&self) -> Command {
        self.command
    }
    pub const fn call_id(&self) -> u64 {
        self.call
    }
    pub const fn observer_state(&self) -> u32 {
        self.state
    }
    pub const fn active_contexts(&self) -> u32 {
        self.active
    }
    pub const fn queued_records(&self) -> u32 {
        self.queued
    }
}

pub fn decode_response(expected: Request, actual_bytes: &[u8]) -> Result<Frame, ProbeError> {
    let frame = decode_frame(actual_bytes)?;
    if frame.command != expected.command
        || frame.session != expected.session.get()
        || frame.call != expected.call.get()
        || frame.maximum != expected.maximum
    {
        return Err(ProbeError::InvalidArchive);
    }
    Ok(frame)
}

/// Static parsing deliberately permits BEGIN/call2: it is inconsistent archive
/// history, not an excuse to discard a complete well-shaped received frame.
pub(crate) fn decode_frame(bytes: &[u8]) -> Result<Frame, ProbeError> {
    use ProbeError::InvalidArchive;
    if bytes.len() < HEADER_BYTES
        || &bytes[..8] != b"WCHP04RS"
        || get32(bytes, 8)? != 1
        || get32(bytes, 12)? != 144
        || get32(bytes, 20)? != 96
    {
        return Err(InvalidArchive);
    }
    let count = get32(bytes, 24)?;
    if count > 16
        || bytes.len() != HEADER_BYTES + RECORD_BYTES * count as usize
        || get32(bytes, 16)? as usize != bytes.len()
    {
        return Err(InvalidArchive);
    }
    let command = Command::from_tag(get32(bytes, 28)?)?;
    let maximum = get32(bytes, 112)?;
    check_maximum(command, maximum)?;
    if count > maximum || (command != Command::Drain && count != 0) {
        return Err(InvalidArchive);
    }
    let session = get64(bytes, 32)?;
    let call = get64(bytes, 48)?;
    let state = get32(bytes, 56)?;
    let pnp = get32(bytes, 60)?;
    let active = get32(bytes, 64)?;
    let queued = get32(bytes, 68)?;
    let known = get32(bytes, 72)?;
    let faults = get32(bytes, 76)?;
    if session == 0
        || get64(bytes, 40)? != 1
        || call == 0
        || !(1..=3).contains(&state)
        || pnp > 6
        || active > 64
        || queued > 256
        || known & !7 != 0
        || faults & !0x1ff != 0
        || !zero(bytes, 116..144)
    {
        return Err(InvalidArchive);
    }
    let attempted = counter(bytes, 80, known & 1 != 0)?;
    let dropped = counter(bytes, 88, known & 1 != 0)?;
    let popped = counter(bytes, 96, known & 2 != 0)?;
    let skipped = counter(bytes, 104, known & 4 != 0)?;
    let records = bytes[HEADER_BYTES..]
        .as_chunks::<RECORD_BYTES>()
        .0
        .iter()
        .map(|record| decode_record(record))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Frame {
        command,
        session,
        call,
        maximum,
        state,
        pnp,
        active,
        queued,
        known,
        faults,
        attempted,
        dropped,
        popped,
        skipped,
        records,
    })
}

fn counter(bytes: &[u8], at: usize, known: bool) -> Result<Option<u64>, ProbeError> {
    let value = get64(bytes, at)?;
    match (known, value) {
        (true, n) => Ok(Some(n)),
        (false, 0) => Ok(None),
        _ => Err(ProbeError::InvalidArchive),
    }
}

fn decode_record(bytes: &[u8]) -> Result<Record, ProbeError> {
    use ProbeError::InvalidArchive;
    let seq = get64(bytes, 0)?;
    let request_id = get64(bytes, 8)?;
    if request_id == 0 || get32(bytes, 20)? != 0 || !zero(bytes, 80..96) {
        return Err(InvalidArchive);
    }
    let observation = match get32(bytes, 16)? {
        1 => {
            if !zero(bytes, 64..80) {
                return Err(InvalidArchive);
            }
            let raw = SubmitSnapshot {
                version: 1,
                major: get32(bytes, 24)?,
                minor: get32(bytes, 28)?,
                flags: get32(bytes, 32)?,
                control_known: get32(bytes, 36)?,
                range_known: get32(bytes, 40)?,
                control_code: get32(bytes, 44)?,
                reserved: 0,
                offset: get64(bytes, 48)?,
                length: get64(bytes, 56)?,
            };
            Observation::Submit(decode_submit(raw).map_err(|_| InvalidArchive)?)
        }
        2 => {
            if !zero(bytes, 24..64) {
                return Err(InvalidArchive);
            }
            let status = get32(bytes, 64)?;
            let known = get32(bytes, 68)?;
            let raw = get64(bytes, 72)?;
            let information = match (known, status, raw) {
                (1, 0, n) => Some(n),
                (0, _, 0) => None,
                _ => return Err(InvalidArchive),
            };
            // Saved-operation consistency is a history check, not static shape.
            Observation::Complete(Completion {
                status,
                information,
            })
        }
        _ => return Err(InvalidArchive),
    };
    Ok(Record {
        seq,
        request_id,
        observation,
    })
}

pub(crate) fn get32(bytes: &[u8], at: usize) -> Result<u32, ProbeError> {
    let array: [u8; 4] = bytes
        .get(at..at + 4)
        .ok_or(ProbeError::InvalidArchive)?
        .try_into()
        .map_err(|_| ProbeError::InvalidArchive)?;
    Ok(u32::from_le_bytes(array))
}
pub(crate) fn get64(bytes: &[u8], at: usize) -> Result<u64, ProbeError> {
    let array: [u8; 8] = bytes
        .get(at..at + 8)
        .ok_or(ProbeError::InvalidArchive)?
        .try_into()
        .map_err(|_| ProbeError::InvalidArchive)?;
    Ok(u64::from_le_bytes(array))
}
pub(crate) fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
pub(crate) fn put64(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
pub(crate) fn zero(bytes: &[u8], range: std::ops::Range<usize>) -> bool {
    bytes.get(range).is_some_and(|s| s.iter().all(|b| *b == 0))
}
