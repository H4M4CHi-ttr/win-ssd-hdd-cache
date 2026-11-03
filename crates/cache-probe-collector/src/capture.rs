//! Safe finite capture orchestration. The Windows transport is separately gated.

use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectError {
    OutputIo,
    #[cfg(any(test, all(windows, feature = "experimental-live")))]
    RngUnavailable,
    #[cfg(any(test, all(windows, feature = "experimental-live")))]
    DeviceUnavailable,
    #[cfg(any(test, all(windows, feature = "experimental-live")))]
    Usage,
    #[cfg(any(test, all(windows, feature = "experimental-live")))]
    InvalidArchive,
}

impl CollectError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::OutputIo => "PROBE_OUTPUT_IO",
            #[cfg(any(test, all(windows, feature = "experimental-live")))]
            Self::RngUnavailable => "PROBE_RNG_UNAVAILABLE",
            #[cfg(any(test, all(windows, feature = "experimental-live")))]
            Self::DeviceUnavailable => "PROBE_DEVICE_UNAVAILABLE",
            #[cfg(any(test, all(windows, feature = "experimental-live")))]
            Self::Usage => "PROBE_USAGE",
            #[cfg(any(test, all(windows, feature = "experimental-live")))]
            Self::InvalidArchive => "INVALID_PROBE_ARCHIVE",
        }
    }
}

/// Writes in byte order, stops at the first error, and never claims durability.
/// In particular, a failing prefix write cannot be followed by a footer write.
pub fn emit_archive(writer: &mut impl Write, bytes: &[u8]) -> Result<(), CollectError> {
    writer
        .write_all(bytes)
        .map_err(|_| CollectError::OutputIo)?;
    writer.flush().map_err(|_| CollectError::OutputIo)
}

#[cfg(any(test, all(windows, feature = "experimental-live")))]
pub use runtime::{
    CaptureClock, ProbeTransport, SessionError, SessionSource, TransportError, collect,
    fresh_session,
};

#[cfg(any(test, all(windows, feature = "experimental-live")))]
mod runtime {
    use super::CollectError;
    use cache_probe_core::{CaptureBuilder, CollectorFault, Command, Request};
    use std::num::NonZeroU64;

    #[derive(Debug, Clone, Copy)]
    pub struct TransportError;
    #[derive(Debug, Clone, Copy)]
    pub struct SessionError;
    pub trait ProbeTransport {
        fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError>;
    }
    pub trait CaptureClock {
        fn elapsed_ms(&self) -> u64;
        fn wait_ms(&mut self, milliseconds: u32);
    }
    pub trait SessionSource {
        fn draw(&mut self) -> Result<u64, SessionError>;
    }

    pub fn fresh_session(source: &mut impl SessionSource) -> Result<NonZeroU64, CollectError> {
        for _ in 0..4 {
            let value = source.draw().map_err(|_| CollectError::RngUnavailable)?;
            if let Some(session) = NonZeroU64::new(value) {
                return Ok(session);
            }
        }
        Err(CollectError::RngUnavailable)
    }

    fn exchange(
        builder: &mut CaptureBuilder,
        transport: &mut impl ProbeTransport,
        command: Command,
        maximum: u32,
    ) -> bool {
        let Ok(request) = builder.next_request(command, maximum) else {
            return false;
        };
        match transport.exchange(request) {
            Ok(bytes) => builder.receive(request, &bytes).is_ok(),
            Err(_) => {
                let _ = builder.fail(request, CollectorFault::TransportError);
                false
            }
        }
    }

    pub fn collect(
        transport: &mut impl ProbeTransport,
        clock: &mut impl CaptureClock,
        session: NonZeroU64,
        duration_ms: u32,
    ) -> Result<Vec<u8>, CollectError> {
        if !(1..=30_000).contains(&duration_ms) {
            return Err(CollectError::Usage);
        }
        let mut builder = CaptureBuilder::new(session);
        if !exchange(&mut builder, transport, Command::Begin, 0) {
            return Err(CollectError::DeviceUnavailable);
        }
        let start = clock.elapsed_ms();
        while clock.elapsed_ms().saturating_sub(start) < u64::from(duration_ms) {
            if !builder.normal_draining_allowed() {
                break;
            }
            if builder.remaining_frames() <= 2 || builder.remaining_records() == 0 {
                builder.latch_fault(CollectorFault::LimitReached);
                break;
            }
            let maximum = builder.remaining_records().min(16) as u32;
            if !exchange(&mut builder, transport, Command::Drain, maximum) {
                break;
            }
            if builder
                .last_frame()
                .is_some_and(|frame| frame.records().is_empty())
            {
                clock.wait_ms(10);
            }
        }
        // After uncertainty this is the sole fresh best-effort cleanup command.
        let stopped = exchange(&mut builder, transport, Command::Stop, 0);
        if !stopped {
            // If this was the original normal STOP, one fresh cleanup STOP is
            // still permitted. Builder refuses if it was already that cleanup.
            exchange(&mut builder, transport, Command::Stop, 0);
        }
        let stopped_at = clock.elapsed_ms();
        while stopped && builder.normal_draining_allowed() && !builder.terminal_received() {
            if clock.elapsed_ms().saturating_sub(stopped_at) >= 5000 {
                builder.latch_fault(CollectorFault::TerminalTimeout);
                break;
            }
            if builder.remaining_frames() == 0 {
                builder.latch_fault(CollectorFault::LimitReached);
                break;
            }
            if builder.remaining_records() == 0 {
                builder.latch_fault(CollectorFault::LimitReached);
                // A metadata-only status is legal; Drain(maximum0) never is.
                if !exchange(&mut builder, transport, Command::Status, 0) {
                    exchange(&mut builder, transport, Command::Stop, 0);
                }
                break;
            }
            let maximum = builder.remaining_records().min(16) as u32;
            if !exchange(&mut builder, transport, Command::Drain, maximum) {
                // This failure may occur after a previously acknowledged STOP.
                // One new STOP is permitted, but no further DRAIN is permitted.
                exchange(&mut builder, transport, Command::Stop, 0);
                break;
            }
            if builder
                .last_frame()
                .is_some_and(|frame| frame.records().is_empty())
            {
                clock.wait_ms(10);
            }
        }
        builder.finish().map_err(|_| CollectError::InvalidArchive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cache_probe_core::{Command, Request, audit_archive, decode_archive};
    use std::{collections::VecDeque, io, num::NonZeroU64};

    fn literal() -> Vec<u8> {
        let h: String = include_str!("../../../tests/fixtures/capture-probe-v1/read-pair.hex")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        h.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect()
    }
    fn put32(b: &mut [u8], at: usize, n: u32) {
        b[at..at + 4].copy_from_slice(&n.to_le_bytes());
    }
    fn put64(b: &mut [u8], at: usize, n: u64) {
        b[at..at + 8].copy_from_slice(&n.to_le_bytes());
    }
    fn session() -> NonZeroU64 {
        NonZeroU64::new(7).unwrap()
    }
    struct Rng {
        draws: VecDeque<Result<u64, SessionError>>,
        count: usize,
    }
    impl SessionSource for Rng {
        fn draw(&mut self) -> Result<u64, SessionError> {
            self.count += 1;
            self.draws.pop_front().expect("unexpected RNG draw")
        }
    }
    struct Clock(u64);
    impl CaptureClock for Clock {
        fn elapsed_ms(&self) -> u64 {
            self.0
        }
        fn wait_ms(&mut self, ms: u32) {
            self.0 += u64::from(ms);
        }
    }
    type ExchangeRow = (Command, u64, u32, Result<Vec<u8>, TransportError>);
    struct Script {
        rows: VecDeque<ExchangeRow>,
        calls: Vec<(Command, u64, u32)>,
    }
    impl ProbeTransport for Script {
        fn exchange(&mut self, req: Request) -> Result<Vec<u8>, TransportError> {
            let row = self.rows.pop_front().expect("unplanned transport call");
            let actual = (req.command(), req.call_id(), req.maximum());
            assert_eq!(actual, (row.0, row.1, row.2));
            assert_eq!(req.session_id(), 7);
            self.calls.push(actual);
            row.3
        }
    }
    fn scripted(rows: Vec<ExchangeRow>) -> Script {
        Script {
            rows: rows.into(),
            calls: vec![],
        }
    }
    fn empty(command: Command, call: u64, state: u32, active: u32) -> Vec<u8> {
        let mut b = literal()[64..208].to_vec();
        put32(&mut b, 28, command as u32);
        put64(&mut b, 48, call);
        put32(&mut b, 56, state);
        put32(&mut b, 64, active);
        put32(&mut b, 112, if command == Command::Drain { 16 } else { 0 });
        b
    }

    #[test]
    fn rng_four_zero_draws() {
        let mut rng = Rng {
            draws: [Ok(0), Ok(0), Ok(0), Ok(0)].into(),
            count: 0,
        };
        assert_eq!(
            fresh_session(&mut rng).unwrap_err().code(),
            "PROBE_RNG_UNAVAILABLE"
        );
        assert_eq!(rng.count, 4);
    }
    #[test]
    fn rng_zero_then_seven() {
        let mut rng = Rng {
            draws: [Ok(0), Ok(7)].into(),
            count: 0,
        };
        assert_eq!(fresh_session(&mut rng).unwrap().get(), 7);
        assert_eq!(rng.count, 2);
        let mut rng = Rng {
            draws: [Err(SessionError)].into(),
            count: 0,
        };
        assert_eq!(
            fresh_session(&mut rng).unwrap_err().code(),
            "PROBE_RNG_UNAVAILABLE"
        );
        assert_eq!(rng.count, 1);
    }
    #[test]
    fn failed_begin_emits_nothing() {
        let mut transport = scripted(vec![(Command::Begin, 1, 0, Err(TransportError))]);
        assert_eq!(
            collect(&mut transport, &mut Clock(0), session(), 1)
                .unwrap_err()
                .code(),
            "PROBE_DEVICE_UNAVAILABLE"
        );
        assert!(transport.rows.is_empty());
    }
    #[test]
    fn live_first_drain_failure_stops_normal_collection() {
        let mut transport = scripted(vec![
            (Command::Begin, 1, 0, Ok(empty(Command::Begin, 1, 1, 0))),
            (Command::Drain, 2, 16, Err(TransportError)),
            (Command::Stop, 3, 0, Ok(empty(Command::Stop, 3, 3, 0))),
        ]);
        let b = collect(&mut transport, &mut Clock(0), session(), 1).unwrap();
        assert!(transport.rows.is_empty());
        assert_eq!(transport.calls.len(), 3);
        let archive = decode_archive(&b).unwrap();
        assert_eq!(archive.frames().len(), 2);
        assert_eq!(archive.record_count(), 0);
        let report = audit_archive(&b).unwrap().to_json();
        assert!(report.contains("\"COLLECTOR_FAULT\""));
        assert!(report.contains("\"CALL_SEQUENCE_GAP\""));
        assert!(report.contains("\"retained_records_reconciled\":false"));
    }
    #[test]
    fn mismatched_call_id_stops() {
        let mut bad = empty(Command::Drain, 99, 1, 0);
        put32(&mut bad, 112, 16);
        let mut transport = scripted(vec![
            (Command::Begin, 1, 0, Ok(empty(Command::Begin, 1, 1, 0))),
            (Command::Drain, 2, 16, Ok(bad)),
            (Command::Stop, 3, 0, Err(TransportError)),
        ]);
        let b = collect(&mut transport, &mut Clock(0), session(), 1).unwrap();
        assert!(transport.rows.is_empty());
        assert_eq!(decode_archive(&b).unwrap().frames().len(), 1);
        let end = b.len() - 64;
        assert_eq!(
            u32::from_le_bytes(b[end + 24..end + 28].try_into().unwrap()),
            3
        );
    }
    #[test]
    fn deadline_keeps_late_requests_unresolved() {
        struct Pending {
            calls: u64,
        }
        impl ProbeTransport for Pending {
            fn exchange(&mut self, req: Request) -> Result<Vec<u8>, TransportError> {
                self.calls += 1;
                let (state, active) = if req.command() == Command::Begin {
                    (1, 0)
                } else {
                    (2, 1)
                };
                Ok(empty(req.command(), req.call_id(), state, active))
            }
        }
        let mut clock = Clock(0);
        let mut transport = Pending { calls: 0 };
        let b = collect(&mut transport, &mut clock, session(), 1).unwrap();
        assert!(clock.0 >= 5000 && clock.0 <= 5020);
        let archive = decode_archive(&b).unwrap();
        assert_eq!(archive.frames().last().unwrap().active_contexts(), 1);
        let end = b.len() - 64;
        assert_eq!(
            u32::from_le_bytes(b[end + 24..end + 28].try_into().unwrap()),
            8
        );
        assert!(!audit_archive(&b).unwrap().terminal_received());
    }
    #[test]
    fn budget_stops_without_truncation() {
        struct Full {
            records: u64,
            stopped: bool,
            calls: u64,
        }
        impl ProbeTransport for Full {
            fn exchange(&mut self, req: Request) -> Result<Vec<u8>, TransportError> {
                self.calls += 1;
                if req.command() == Command::Begin {
                    return Ok(empty(Command::Begin, 1, 1, 0));
                }
                if req.command() == Command::Stop {
                    self.stopped = true;
                    let mut b = empty(Command::Stop, req.call_id(), 3, 0);
                    put64(&mut b, 80, self.records);
                    put64(&mut b, 96, self.records);
                    return Ok(b);
                }
                assert!(!self.stopped);
                assert_eq!(req.command(), Command::Drain);
                assert_eq!(req.maximum(), 16);
                let mut b = literal()[208..352].to_vec();
                put32(&mut b, 16, 1680);
                put32(&mut b, 24, 16);
                put64(&mut b, 48, req.call_id());
                for i in 0..8 {
                    let mut pair = literal()[352..544].to_vec();
                    let base = self.records + i * 2;
                    put64(&mut pair, 0, base);
                    put64(&mut pair, 8, base / 2 + 1);
                    put64(&mut pair, 96, base + 1);
                    put64(&mut pair, 104, base / 2 + 1);
                    b.extend(pair);
                }
                self.records += 16;
                put64(&mut b, 80, self.records);
                put64(&mut b, 96, self.records);
                Ok(b)
            }
        }
        let mut transport = Full {
            records: 0,
            stopped: false,
            calls: 0,
        };
        let b = collect(&mut transport, &mut Clock(0), session(), 30_000).unwrap();
        assert!(transport.stopped);
        assert_eq!(transport.calls, 258);
        assert_eq!(decode_archive(&b).unwrap().record_count(), 4096);
        let end = b.len() - 64;
        assert_eq!(
            u32::from_le_bytes(b[end + 24..end + 28].try_into().unwrap()),
            4
        );
        assert!(!audit_archive(&b).unwrap().retained_records_reconciled());
    }
    struct FailWriter {
        left: usize,
        bytes: Vec<u8>,
        flush_error: bool,
    }
    impl io::Write for FailWriter {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            let n = b.len().min(self.left);
            if n == 0 {
                return Err(io::Error::other("sensitive output detail"));
            }
            self.bytes.extend_from_slice(&b[..n]);
            self.left -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.flush_error {
                Err(io::Error::other("flush detail"))
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn output_failure_has_no_success_claim() {
        let b = literal();
        for (left, flush_error) in [(0, false), (688, false), (700, false), (752, true)] {
            let mut sink = FailWriter {
                left,
                bytes: vec![],
                flush_error,
            };
            assert_eq!(
                emit_archive(&mut sink, &b).unwrap_err().code(),
                "PROBE_OUTPUT_IO"
            );
            assert_eq!(sink.bytes.len(), left);
            if left == 752 {
                assert_eq!(&sink.bytes[740..744], &[0, 0, 0, 0]);
            }
        }
    }

    #[test]
    fn original_stop_failure_gets_one_cleanup_stop() {
        let mut transport = scripted(vec![
            (Command::Begin, 1, 0, Ok(empty(Command::Begin, 1, 1, 0))),
            (Command::Drain, 2, 16, Ok(empty(Command::Drain, 2, 1, 0))),
            (Command::Stop, 3, 0, Err(TransportError)),
            (Command::Stop, 4, 0, Err(TransportError)),
        ]);
        let b = collect(&mut transport, &mut Clock(0), session(), 1).unwrap();
        assert!(transport.rows.is_empty());
        assert_eq!(transport.calls.len(), 4);
        assert_eq!(decode_archive(&b).unwrap().frames().len(), 2);
    }

    #[test]
    fn budget_status_failure_gets_one_cleanup_stop() {
        struct Budget {
            retained: u64,
            calls: u64,
            stops: usize,
        }
        impl ProbeTransport for Budget {
            fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
                self.calls += 1;
                assert_eq!(request.call_id(), self.calls);
                match request.command() {
                    Command::Begin => Ok(empty(Command::Begin, 1, 1, 0)),
                    Command::Drain => {
                        assert_eq!(request.maximum(), 16);
                        let mut b = literal()[208..352].to_vec();
                        put32(&mut b, 16, 1680);
                        put32(&mut b, 24, 16);
                        put64(&mut b, 48, request.call_id());
                        for pair_index in 0..8 {
                            let seq = self.retained + pair_index * 2;
                            let mut pair = literal()[352..544].to_vec();
                            put64(&mut pair, 0, seq);
                            put64(&mut pair, 8, seq / 2 + 1);
                            put64(&mut pair, 96, seq + 1);
                            put64(&mut pair, 104, seq / 2 + 1);
                            b.extend(pair);
                        }
                        self.retained += 16;
                        put64(&mut b, 80, self.retained);
                        put64(&mut b, 96, self.retained);
                        Ok(b)
                    }
                    Command::Status => {
                        assert_eq!(request.call_id(), 259);
                        Err(TransportError)
                    }
                    Command::Stop => {
                        self.stops += 1;
                        let (state, active, queued, attempted) = if self.stops == 1 {
                            // One new submit reached the ring between the last
                            // full drain and STOP. Its completion arrives later.
                            (2, 1, 1, 4097)
                        } else {
                            (3, 0, 2, 4098)
                        };
                        let mut b = empty(Command::Stop, request.call_id(), state, active);
                        put32(&mut b, 68, queued);
                        put64(&mut b, 80, attempted);
                        put64(&mut b, 96, 4096);
                        Ok(b)
                    }
                }
            }
        }
        let mut transport = Budget {
            retained: 0,
            calls: 0,
            stops: 0,
        };
        let bytes = collect(&mut transport, &mut Clock(0), session(), 30_000).unwrap();
        assert_eq!(transport.calls, 260);
        assert_eq!(transport.stops, 2);
        let archive = decode_archive(&bytes).unwrap();
        assert_eq!(archive.record_count(), 4096);
        assert_eq!(archive.frames().last().unwrap().queued_records(), 2);
        let end = bytes.len() - 64;
        // LIMIT_REACHED4 | TRANSPORT_ERROR1 | missing call259 history fault16.
        assert_eq!(
            u32::from_le_bytes(bytes[end + 24..end + 28].try_into().unwrap()),
            21
        );
    }
}
