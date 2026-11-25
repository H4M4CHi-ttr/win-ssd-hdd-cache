//! Safe finite capture orchestration. The Windows transport is separately gated.

use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectError {
    OutputIo,
    AbortControl,
    RngUnavailable,
    DeviceUnavailable,
    Usage,
    InvalidArchive,
}

impl CollectError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::OutputIo => "PROBE_OUTPUT_IO",
            Self::AbortControl => "PROBE_ABORT_CONTROL_IO",
            Self::RngUnavailable => "PROBE_RNG_UNAVAILABLE",
            Self::DeviceUnavailable => "PROBE_DEVICE_UNAVAILABLE",
            Self::Usage => "PROBE_USAGE",
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

pub use runtime::{
    AbortPollError, AbortSignal, CaptureClock, ControlledOutcome, ControlledReport, ProbeTransport,
    SessionError, SessionSource, StopReason, TransportError, collect, collect_controlled,
    fresh_session,
};

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

    /// Priorities apply within a boundary; the first selected reason is sticky.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum StopReason {
        Uncertain,
        ControlFailure,
        Producer,
        ExternalAbort,
        Limit,
        Duration,
        TerminalTimeout,
    }
    impl StopReason {
        pub const fn code(self) -> &'static str {
            match self {
                Self::Uncertain => "UNCERTAIN",
                Self::ControlFailure => "CONTROL_FAILURE",
                Self::Producer => "PRODUCER",
                Self::ExternalAbort => "EXTERNAL_ABORT",
                Self::Limit => "LIMIT",
                Self::Duration => "DURATION",
                Self::TerminalTimeout => "TERMINAL_TIMEOUT",
            }
        }
    }
    #[derive(Debug, Clone, Copy)]
    pub struct AbortPollError;
    pub trait AbortSignal {
        /// A nonblocking query. Never writes diagnostics or issues device I/O.
        fn poll(&mut self) -> Result<bool, AbortPollError>;
    }
    #[derive(Debug, Clone, Copy, Default)]
    pub struct ControlledReport {
        pub first_reason: Option<StopReason>,
        pub reasons: u16,
        pub abort_accepted: bool,
        pub begin_submitted: bool,
        pub begin_acknowledged: Option<bool>,
        pub stop_acknowledged: Option<bool>,
        pub terminal_received: Option<bool>,
    }
    impl ControlledReport {
        pub const fn has_reason(&self, reason: StopReason) -> bool {
            self.reasons & (1 << reason as u8) != 0
        }
        fn add(&mut self, reason: StopReason) {
            self.reasons |= 1 << reason as u8;
        }
        fn select(&mut self) {
            if self.first_reason.is_none() {
                self.first_reason = [
                    StopReason::Uncertain,
                    StopReason::ControlFailure,
                    StopReason::Producer,
                    StopReason::ExternalAbort,
                    StopReason::Limit,
                    StopReason::Duration,
                    StopReason::TerminalTimeout,
                ]
                .into_iter()
                .find(|reason| self.has_reason(*reason));
            }
        }
        /// Emitted only after the transport is dropped. Unknown external facts
        /// remain null; the receiver must record its own hash and save result.
        pub fn to_json(&self) -> String {
            fn fact(value: Option<bool>) -> &'static str {
                match value {
                    Some(true) => "true",
                    Some(false) => "false",
                    None => "null",
                }
            }
            let reason = self
                .first_reason
                .map_or("null".to_owned(), |r| format!("\"{}\"", r.code()));
            let reasons = [
                StopReason::Uncertain,
                StopReason::ControlFailure,
                StopReason::Producer,
                StopReason::ExternalAbort,
                StopReason::Limit,
                StopReason::Duration,
                StopReason::TerminalTimeout,
            ]
            .into_iter()
            .filter(|r| self.has_reason(*r))
            .map(|r| format!("\"{}\"", r.code()))
            .collect::<Vec<_>>()
            .join(",");
            format!(
                "{{\"schema\":\"p04-controlled-v1\",\"first_reason\":{reason},\"reasons\":[{reasons}],\"abort_accepted\":{},\"begin_submitted\":{},\"begin_acknowledged\":{},\"stop_acknowledged\":{},\"terminal_received\":{},\"send_result\":null,\"host_deadline\":null,\"process_exit\":null,\"received_bytes\":null,\"received_hash\":null,\"saved\":null}}\n",
                self.abort_accepted,
                self.begin_submitted,
                fact(self.begin_acknowledged),
                fact(self.stop_acknowledged),
                fact(self.terminal_received)
            )
        }
    }
    #[derive(Debug)]
    pub struct ControlledOutcome {
        /// None means an explicit abort before BEGIN, with no archive (exit 5).
        pub archive: Result<Option<Vec<u8>>, CollectError>,
        pub report: ControlledReport,
    }
    struct Control<'a> {
        signal: Option<&'a mut dyn AbortSignal>,
        report: ControlledReport,
    }
    impl Control<'_> {
        fn poll(&mut self) {
            let Some(signal) = &mut self.signal else {
                return;
            };
            match signal.poll() {
                Ok(true) => {
                    self.report.abort_accepted = true;
                    self.report.add(StopReason::ExternalAbort);
                }
                Ok(false) => {}
                Err(_) => self.report.add(StopReason::ControlFailure),
            }
        }
        fn before_begin(&mut self) -> Result<bool, CollectError> {
            self.poll();
            self.report.select();
            if self.report.has_reason(StopReason::ControlFailure) {
                Err(CollectError::AbortControl)
            } else {
                Ok(self.report.abort_accepted)
            }
        }
        fn boundary(
            &mut self,
            builder: &CaptureBuilder,
            normal: bool,
            failed: bool,
            duration: bool,
        ) -> bool {
            if self.signal.is_none() {
                return false;
            }
            if failed || !builder.normal_draining_allowed() {
                self.report.add(StopReason::Uncertain);
            }
            // A valid terminal ends control processing. Late notifications belong
            // to the supervisor and must not reopen or mutate this capture.
            if !builder.terminal_received() {
                self.poll();
            }
            if let Some(frame) = builder.last_frame()
                && (frame.producer_faults() != 0
                    || frame.known_counters() != 7
                    || frame.dropped_records().is_some_and(|n| n != 0)
                    || frame.skipped_records().is_some_and(|n| n != 0)
                    || (normal && frame.observer_state() != 1))
            {
                self.report.add(StopReason::Producer);
            }
            if normal && (builder.remaining_frames() <= 2 || builder.remaining_records() == 0) {
                self.report.add(StopReason::Limit);
            }
            if duration {
                self.report.add(StopReason::Duration);
            }
            self.report.select();
            self.report.first_reason.is_some()
        }
        fn received(&mut self, builder: &CaptureBuilder, command: Command, valid: bool) {
            if self.signal.is_none() || !valid {
                return;
            }
            if command == Command::Begin {
                self.report.begin_acknowledged = Some(true);
            }
            if command == Command::Stop {
                self.report.stop_acknowledged = builder
                    .last_frame()
                    .map(|frame| matches!(frame.observer_state(), 2 | 3));
            }
            self.report.terminal_received = Some(builder.terminal_received());
        }
        fn exchange(
            &mut self,
            builder: &mut CaptureBuilder,
            transport: &mut impl ProbeTransport,
            command: Command,
            maximum: u32,
        ) -> bool {
            let valid = exchange(builder, transport, command, maximum);
            self.received(builder, command, valid);
            valid
        }
    }
    pub fn collect_controlled<T: ProbeTransport>(
        open: impl FnOnce() -> Result<T, TransportError>,
        clock: &mut impl CaptureClock,
        session: NonZeroU64,
        duration_ms: u32,
        signal: &mut impl AbortSignal,
    ) -> ControlledOutcome {
        let mut control = Control {
            signal: Some(signal),
            report: ControlledReport::default(),
        };
        let archive = (|| {
            if !(1..=30_000).contains(&duration_ms) {
                return Err(CollectError::Usage);
            }
            if control.before_begin()? {
                return Ok(None);
            }
            // The same transport remains owned until all synchronous exchanges
            // return. No logger, second owner or cancellation thread receives it.
            let mut transport = open().map_err(|_| CollectError::DeviceUnavailable)?;
            if control.before_begin()? {
                return Ok(None);
            }
            collect_inner(&mut transport, clock, session, duration_ms, &mut control).map(Some)
        })();
        ControlledOutcome {
            archive,
            report: control.report,
        }
    }
    pub fn collect(
        transport: &mut impl ProbeTransport,
        clock: &mut impl CaptureClock,
        session: NonZeroU64,
        duration_ms: u32,
    ) -> Result<Vec<u8>, CollectError> {
        collect_inner(
            transport,
            clock,
            session,
            duration_ms,
            &mut Control {
                signal: None,
                report: ControlledReport::default(),
            },
        )
    }
    fn collect_inner(
        transport: &mut impl ProbeTransport,
        clock: &mut impl CaptureClock,
        session: NonZeroU64,
        duration_ms: u32,
        control: &mut Control<'_>,
    ) -> Result<Vec<u8>, CollectError> {
        if !(1..=30_000).contains(&duration_ms) {
            return Err(CollectError::Usage);
        }
        let mut builder = CaptureBuilder::new(session);
        control.report.begin_submitted = true;
        if !control.exchange(&mut builder, transport, Command::Begin, 0) {
            control.boundary(&builder, true, true, false);
            return Err(CollectError::DeviceUnavailable);
        }
        let start = clock.elapsed_ms();
        control.boundary(&builder, true, false, false);
        while clock.elapsed_ms().saturating_sub(start) < u64::from(duration_ms) {
            if control.boundary(&builder, true, false, false) || !builder.normal_draining_allowed()
            {
                break;
            }
            if builder.remaining_frames() <= 2 || builder.remaining_records() == 0 {
                builder.latch_fault(CollectorFault::LimitReached);
                break;
            }
            let maximum = builder.remaining_records().min(16) as u32;
            let valid = control.exchange(&mut builder, transport, Command::Drain, maximum);
            let expired = clock.elapsed_ms().saturating_sub(start) >= u64::from(duration_ms);
            if control.boundary(&builder, true, !valid, expired) || !valid {
                break;
            }
            if builder
                .last_frame()
                .is_some_and(|frame| frame.records().is_empty())
            {
                clock.wait_ms(10);
                if control.boundary(
                    &builder,
                    true,
                    false,
                    clock.elapsed_ms().saturating_sub(start) >= u64::from(duration_ms),
                ) {
                    break;
                }
            }
        }
        if control.signal.is_some() {
            control.boundary(
                &builder,
                true,
                false,
                clock.elapsed_ms().saturating_sub(start) >= u64::from(duration_ms),
            );
            if control.report.has_reason(StopReason::Limit) {
                builder.latch_fault(CollectorFault::LimitReached);
            }
        }
        // After uncertainty this is the sole fresh best-effort cleanup command.
        let stop_was_cleanup = !builder.normal_draining_allowed();
        let stopped = control.exchange(&mut builder, transport, Command::Stop, 0);
        control.boundary(&builder, false, !stopped, false);
        if !stopped
            || (control.signal.is_some() && !stop_was_cleanup && !builder.normal_draining_allowed())
        {
            // Builder refuses another attempt when this was already cleanup.
            let valid = control.exchange(&mut builder, transport, Command::Stop, 0);
            control.boundary(&builder, false, !valid, false);
        }
        let stopped_at = clock.elapsed_ms();
        while stopped && builder.normal_draining_allowed() && !builder.terminal_received() {
            if clock.elapsed_ms().saturating_sub(stopped_at) >= 5000 {
                builder.latch_fault(CollectorFault::TerminalTimeout);
                if control.signal.is_some() {
                    control.report.add(StopReason::TerminalTimeout);
                    control.report.select();
                }
                break;
            }
            if builder.remaining_frames() == 0 {
                builder.latch_fault(CollectorFault::LimitReached);
                if control.signal.is_some() {
                    control.report.add(StopReason::Limit);
                }
                break;
            }
            if builder.remaining_records() == 0 {
                builder.latch_fault(CollectorFault::LimitReached);
                if control.signal.is_some() {
                    control.report.add(StopReason::Limit);
                }
                let valid = control.exchange(&mut builder, transport, Command::Status, 0);
                control.boundary(&builder, false, !valid, false);
                if !valid || (control.signal.is_some() && !builder.normal_draining_allowed()) {
                    let valid = control.exchange(&mut builder, transport, Command::Stop, 0);
                    control.boundary(&builder, false, !valid, false);
                }
                break;
            }
            let maximum = builder.remaining_records().min(16) as u32;
            let valid = control.exchange(&mut builder, transport, Command::Drain, maximum);
            control.boundary(&builder, false, !valid, false);
            if !valid || (control.signal.is_some() && !builder.normal_draining_allowed()) {
                let valid = control.exchange(&mut builder, transport, Command::Stop, 0);
                control.boundary(&builder, false, !valid, false);
                break;
            }
            if builder
                .last_frame()
                .is_some_and(|frame| frame.records().is_empty())
            {
                clock.wait_ms(10);
                control.boundary(&builder, false, false, false);
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
        budget_status_failure(false);
    }
    #[test]
    fn c11_controlled_record_budget_status_failure_never_drains_zero() {
        budget_status_failure(true);
    }
    fn budget_status_failure(controlled: bool) {
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
        let bytes = if controlled {
            struct Borrowed<'a>(&'a mut Budget);
            impl ProbeTransport for Borrowed<'_> {
                fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
                    self.0.exchange(request)
                }
            }
            struct Never;
            impl AbortSignal for Never {
                fn poll(&mut self) -> Result<bool, AbortPollError> {
                    Ok(false)
                }
            }
            let outcome = collect_controlled(
                || Ok(Borrowed(&mut transport)),
                &mut Clock(0),
                session(),
                30_000,
                &mut Never,
            );
            assert_eq!(outcome.report.first_reason, Some(StopReason::Limit));
            outcome.archive.unwrap().unwrap()
        } else {
            collect(&mut transport, &mut Clock(0), session(), 30_000).unwrap()
        };
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
    mod controlled {
        use super::*;
        use std::{cell::Cell, rc::Rc};

        struct Signal {
            value: Rc<Cell<u8>>,
            polls: usize,
            race_at: usize,
        }
        impl AbortSignal for Signal {
            fn poll(&mut self) -> Result<bool, AbortPollError> {
                self.polls += 1;
                let value = self.value.get();
                if self.polls == self.race_at {
                    self.value.set(1);
                }
                match value {
                    0 => Ok(false),
                    1 => Ok(true),
                    _ => Err(AbortPollError),
                }
            }
        }
        fn signal(value: u8) -> Signal {
            Signal {
                value: Rc::new(Cell::new(value)),
                polls: 0,
                race_at: usize::MAX,
            }
        }
        struct Notify<'a> {
            script: &'a mut Script,
            signal: Rc<Cell<u8>>,
            call: u64,
            value: u8,
        }
        impl ProbeTransport for Notify<'_> {
            fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
                let result = self.script.exchange(request);
                if request.call_id() == self.call {
                    self.signal.set(self.value);
                }
                result
            }
        }
        fn capture_script(
            script: &mut Script,
            signal: &mut Signal,
            call: u64,
            value: u8,
        ) -> ControlledOutcome {
            let state = signal.value.clone();
            collect_controlled(
                || {
                    Ok(Notify {
                        script,
                        signal: state,
                        call,
                        value,
                    })
                },
                &mut Clock(0),
                session(),
                1,
                signal,
            )
        }
        fn begin() -> ExchangeRow {
            (Command::Begin, 1, 0, Ok(empty(Command::Begin, 1, 1, 0)))
        }
        fn stop(call: u64) -> ExchangeRow {
            (Command::Stop, call, 0, Ok(empty(Command::Stop, call, 3, 0)))
        }
        #[test]
        fn c04_abort_before_open_has_no_transport_or_archive() {
            let opened = Cell::new(0);
            let result = collect_controlled(
                || {
                    opened.set(1);
                    Ok(scripted(vec![]))
                },
                &mut Clock(0),
                session(),
                1,
                &mut signal(1),
            );
            assert_eq!(opened.get(), 0);
            assert!(result.archive.unwrap().is_none());
            assert_eq!(result.report.first_reason, Some(StopReason::ExternalAbort));
            assert!(!result.report.begin_submitted);
            assert_eq!(result.report.begin_acknowledged, None);
            assert_eq!(result.report.terminal_received, None);
        }
        #[test]
        fn c04_abort_after_open_closes_once_without_begin() {
            struct Device(Rc<Cell<u32>>);
            impl Drop for Device {
                fn drop(&mut self) {
                    self.0.set(self.0.get() + 1);
                }
            }
            impl ProbeTransport for Device {
                fn exchange(&mut self, _: Request) -> Result<Vec<u8>, TransportError> {
                    panic!("BEGIN or STOP before acceptance");
                }
            }
            let dropped = Rc::new(Cell::new(0));
            let mut signal = signal(0);
            let state = signal.value.clone();
            let result = collect_controlled(
                || {
                    state.set(1);
                    Ok(Device(dropped.clone()))
                },
                &mut Clock(0),
                session(),
                1,
                &mut signal,
            );
            assert!(result.archive.unwrap().is_none());
            assert_eq!(dropped.get(), 1);
        }
        #[test]
        fn c04_control_failure_before_and_after_open_is_os_error() {
            for after_open in [false, true] {
                let opened = Cell::new(0);
                let mut signal = signal(if after_open { 0 } else { 2 });
                let state = signal.value.clone();
                let result = collect_controlled(
                    || {
                        opened.set(1);
                        state.set(2);
                        Ok(scripted(vec![]))
                    },
                    &mut Clock(0),
                    session(),
                    1,
                    &mut signal,
                );
                assert_eq!(opened.get(), u32::from(after_open));
                assert_eq!(result.archive.unwrap_err(), CollectError::AbortControl);
                assert_eq!(result.report.first_reason, Some(StopReason::ControlFailure));
                assert!(!result.report.begin_submitted);
            }
        }
        #[test]
        fn c05_begin_inflight_abort_waits_for_ack_then_same_owner_stop() {
            let mut script = scripted(vec![begin(), stop(2)]);
            let outcome = capture_script(&mut script, &mut signal(0), 1, 1);
            assert!(outcome.archive.unwrap().is_some());
            assert_eq!(outcome.report.begin_acknowledged, Some(true));
            assert_eq!(outcome.report.stop_acknowledged, Some(true));
            assert_eq!(outcome.report.terminal_received, Some(true));
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c05_unknown_begin_never_retries_or_adds_stop_archive() {
            for reply in [Err(TransportError), Ok(empty(Command::Begin, 99, 1, 0))] {
                let mut script = scripted(vec![(Command::Begin, 1, 0, reply)]);
                let outcome = capture_script(&mut script, &mut signal(0), 1, 1);
                assert_eq!(
                    outcome.archive.unwrap_err(),
                    CollectError::DeviceUnavailable
                );
                assert_eq!(outcome.report.first_reason, Some(StopReason::Uncertain));
                assert!(outcome.report.abort_accepted);
                assert_eq!(outcome.report.begin_acknowledged, None);
                assert_eq!(outcome.report.stop_acknowledged, None);
                assert!(script.rows.is_empty());
            }
        }
        #[test]
        fn c05_history_invalid_begin_uses_only_builder_cleanup_rule() {
            let mut bad = empty(Command::Begin, 1, 1, 0);
            put32(&mut bad, 60, 0);
            let mut script = scripted(vec![(Command::Begin, 1, 0, Ok(bad)), stop(2)]);
            let outcome = capture_script(&mut script, &mut signal(0), 1, 1);
            assert_eq!(outcome.report.first_reason, Some(StopReason::Uncertain));
            let bytes = outcome.archive.unwrap().unwrap();
            assert!(
                audit_archive(&bytes)
                    .unwrap()
                    .to_json()
                    .contains("COLLECTOR_FAULT")
            );
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c06_signal_after_last_poll_allows_one_drain_and_keeps_actual_records_once() {
            let frame = literal()[208..544].to_vec();
            let stop_frame = literal()[544..688].to_vec();
            let mut script = scripted(vec![
                begin(),
                (Command::Drain, 2, 16, Ok(frame)),
                (Command::Stop, 3, 0, Ok(stop_frame)),
            ]);
            let mut signal = signal(0);
            signal.race_at = 4;
            let outcome = capture_script(&mut script, &mut signal, 99, 0);
            assert_eq!(outcome.report.first_reason, Some(StopReason::ExternalAbort));
            let bytes = outcome.archive.unwrap().unwrap();
            assert_eq!(decode_archive(&bytes).unwrap().record_count(), 2);
            assert_eq!(bytes, literal());
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c06_signal_before_predrain_has_zero_normal_drain() {
            let mut script = scripted(vec![begin(), stop(2)]);
            let mut signal = signal(0);
            signal.race_at = 3;
            let outcome = capture_script(&mut script, &mut signal, 99, 0);
            assert!(outcome.report.abort_accepted);
            assert!(outcome.archive.unwrap().is_some());
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c07_drain_uncertainty_wins_over_abort_and_no_second_drain() {
            let invalid = [32, 40, 48].map(|offset| {
                let mut frame = empty(Command::Drain, 2, 1, 0);
                put64(&mut frame, offset, 99);
                Ok(frame)
            });
            for reply in std::iter::once(Err(TransportError)).chain(invalid) {
                let mut script = scripted(vec![begin(), (Command::Drain, 2, 16, reply), stop(3)]);
                let outcome = capture_script(&mut script, &mut signal(0), 2, 1);
                assert_eq!(outcome.report.first_reason, Some(StopReason::Uncertain));
                assert!(outcome.report.has_reason(StopReason::ExternalAbort));
                let bytes = outcome.archive.unwrap().unwrap();
                let report = audit_archive(&bytes).unwrap();
                assert!(!report.retained_records_reconciled());
                assert!(report.to_json().contains("CALL_SEQUENCE_GAP"));
                assert_eq!(decode_archive(&bytes).unwrap().frames().len(), 2);
                assert!(script.rows.is_empty());
            }
        }
        #[test]
        fn c08_failed_normal_stop_has_only_one_fresh_cleanup() {
            for cleanup in [Err(TransportError), Ok(empty(Command::Stop, 4, 3, 0))] {
                let mut script = scripted(vec![
                    begin(),
                    (Command::Drain, 2, 16, Ok(empty(Command::Drain, 2, 1, 0))),
                    (Command::Stop, 3, 0, Err(TransportError)),
                    (Command::Stop, 4, 0, cleanup),
                ]);
                let outcome = capture_script(&mut script, &mut signal(0), 3, 1);
                assert!(outcome.archive.unwrap().is_some());
                assert!(outcome.report.has_reason(StopReason::Uncertain));
                assert!(script.rows.is_empty());
            }
        }
        #[test]
        fn c08_cleanup_stop_failure_does_not_gain_another_budget() {
            let mut script = scripted(vec![
                begin(),
                (Command::Drain, 2, 16, Err(TransportError)),
                (Command::Stop, 3, 0, Err(TransportError)),
            ]);
            let outcome = capture_script(&mut script, &mut signal(0), 2, 1);
            assert!(outcome.archive.unwrap().is_some());
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c08_recovery_drain_failure_has_one_cleanup_and_no_later_drain() {
            let mut script = scripted(vec![
                begin(),
                (Command::Stop, 2, 0, Ok(empty(Command::Stop, 2, 2, 1))),
                (Command::Drain, 3, 16, Err(TransportError)),
                stop(4),
            ]);
            let outcome = capture_script(&mut script, &mut signal(0), 1, 1);
            assert!(outcome.archive.unwrap().is_some());
            assert!(outcome.report.has_reason(StopReason::Uncertain));
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c08_history_invalid_recovery_response_has_only_cleanup_stop() {
            let mut bad = empty(Command::Drain, 3, 2, 1);
            put32(&mut bad, 72, 6);
            let mut cleanup = empty(Command::Stop, 4, 3, 0);
            put32(&mut cleanup, 72, 6);
            let mut script = scripted(vec![
                begin(),
                (Command::Stop, 2, 0, Ok(empty(Command::Stop, 2, 2, 1))),
                (Command::Drain, 3, 16, Ok(bad)),
                (Command::Stop, 4, 0, Ok(cleanup)),
            ]);
            let outcome = capture_script(&mut script, &mut signal(0), 1, 1);
            assert!(outcome.report.has_reason(StopReason::Uncertain));
            assert_eq!(outcome.report.first_reason, Some(StopReason::ExternalAbort));
            assert!(outcome.archive.unwrap().is_some());
            assert!(script.rows.is_empty());
        }
        fn producer_case(kind: u8, concurrent_abort: bool) {
            let mut drain = empty(Command::Drain, 2, 1, 0);
            match kind {
                0 => put32(&mut drain, 76, 1),
                1 => {
                    put64(&mut drain, 80, 1);
                    put64(&mut drain, 88, 1);
                }
                2 => put64(&mut drain, 104, 1),
                3..=5 => {
                    let (known, fault) = [(6, 4), (5, 128), (3, 64)][usize::from(kind - 3)];
                    put32(&mut drain, 72, known);
                    put32(&mut drain, 76, fault);
                }
                6 => put32(&mut drain, 56, 2),
                _ => unreachable!(),
            }
            let mut stopped = drain.clone();
            put32(&mut stopped, 28, Command::Stop as u32);
            put64(&mut stopped, 48, 3);
            put32(&mut stopped, 56, 3);
            put32(&mut stopped, 112, 0);
            let mut script = scripted(vec![
                begin(),
                (Command::Drain, 2, 16, Ok(drain.clone())),
                (Command::Stop, 3, 0, Ok(stopped)),
            ]);
            let outcome =
                capture_script(&mut script, &mut signal(0), 2, u8::from(concurrent_abort));
            assert_eq!(outcome.report.first_reason, Some(StopReason::Producer));
            assert_eq!(
                outcome.report.has_reason(StopReason::ExternalAbort),
                concurrent_abort
            );
            let bytes = outcome.archive.unwrap().unwrap();
            assert_eq!(&bytes[208..352], drain);
            assert_eq!(decode_archive(&bytes).unwrap().frames().len(), 3);
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c09_producer_fault_stops_normal_drain() {
            producer_case(0, false);
        }
        #[test]
        fn c09_known_drop_stops_normal_drain() {
            producer_case(1, false);
        }
        #[test]
        fn c09_known_skip_stops_normal_drain() {
            producer_case(2, false);
        }
        #[test]
        fn c09_attempted_unknown_remains_unknown() {
            producer_case(3, false);
        }
        #[test]
        fn c09_popped_unknown_remains_unknown() {
            producer_case(4, false);
        }
        #[test]
        fn c09_skipped_unknown_remains_unknown() {
            producer_case(5, false);
        }
        #[test]
        fn c09_running_to_stopping_requires_stop() {
            producer_case(6, false);
        }
        #[test]
        fn c10_producer_priority_preserves_concurrent_abort() {
            producer_case(0, true);
        }
        #[test]
        fn c10_control_poll_failure_after_begin_stops_without_transport_loss() {
            let mut script = scripted(vec![begin(), stop(2)]);
            let outcome = capture_script(&mut script, &mut signal(0), 1, 2);
            assert_eq!(
                outcome.report.first_reason,
                Some(StopReason::ControlFailure)
            );
            let bytes = outcome.archive.unwrap().unwrap();
            let footer = &bytes[bytes.len() - 64..];
            assert_eq!(&footer[24..28], &[0; 4]);
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c10_signal_during_stop_is_secondary_and_never_repeats_normal_stop() {
            let mut script = scripted(vec![
                begin(),
                (Command::Drain, 2, 16, Ok(empty(Command::Drain, 2, 1, 0))),
                (Command::Stop, 3, 0, Ok(empty(Command::Stop, 3, 2, 1))),
                (Command::Drain, 4, 16, Ok(empty(Command::Drain, 4, 3, 0))),
            ]);
            let outcome = capture_script(&mut script, &mut signal(0), 3, 1);
            assert_eq!(outcome.report.first_reason, Some(StopReason::Duration));
            assert!(outcome.report.has_reason(StopReason::ExternalAbort));
            assert_eq!(outcome.report.terminal_received, Some(true));
            assert!(outcome.archive.unwrap().is_some());
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c10_producer_abort_and_expiry_share_one_priority_boundary() {
            struct SharedClock(Rc<Cell<u64>>);
            impl CaptureClock for SharedClock {
                fn elapsed_ms(&self) -> u64 {
                    self.0.get()
                }
                fn wait_ms(&mut self, ms: u32) {
                    self.0.set(self.0.get() + u64::from(ms));
                }
            }
            struct Timed<'a> {
                notify: Notify<'a>,
                now: Rc<Cell<u64>>,
            }
            impl ProbeTransport for Timed<'_> {
                fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
                    if request.command() == Command::Drain {
                        self.now.set(1);
                    }
                    self.notify.exchange(request)
                }
            }
            let mut drain = empty(Command::Drain, 2, 1, 0);
            put32(&mut drain, 76, 1);
            let mut stopped = empty(Command::Stop, 3, 3, 0);
            put32(&mut stopped, 76, 1);
            let mut script = scripted(vec![
                begin(),
                (Command::Drain, 2, 16, Ok(drain)),
                (Command::Stop, 3, 0, Ok(stopped)),
            ]);
            let now = Rc::new(Cell::new(0));
            let mut signal = signal(0);
            let state = signal.value.clone();
            let outcome = collect_controlled(
                || {
                    Ok(Timed {
                        notify: Notify {
                            script: &mut script,
                            signal: state,
                            call: 2,
                            value: 1,
                        },
                        now: now.clone(),
                    })
                },
                &mut SharedClock(now.clone()),
                session(),
                1,
                &mut signal,
            );
            assert_eq!(outcome.report.first_reason, Some(StopReason::Producer));
            assert!(outcome.report.has_reason(StopReason::ExternalAbort));
            assert!(outcome.report.has_reason(StopReason::Duration));
            assert!(outcome.archive.unwrap().is_some());
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c11_post_stop_deadline_is_checked_only_on_returned_boundaries() {
            struct Pending {
                stops: u32,
            }
            impl ProbeTransport for Pending {
                fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
                    if request.command() == Command::Stop {
                        self.stops += 1;
                    }
                    let (state, active) = if request.command() == Command::Begin {
                        (1, 0)
                    } else {
                        (2, 1)
                    };
                    Ok(empty(request.command(), request.call_id(), state, active))
                }
            }
            struct Borrowed<'a>(&'a mut Pending);
            impl ProbeTransport for Borrowed<'_> {
                fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
                    self.0.exchange(request)
                }
            }
            let mut transport = Pending { stops: 0 };
            let mut clock = Clock(0);
            let outcome = collect_controlled(
                || Ok(Borrowed(&mut transport)),
                &mut clock,
                session(),
                1,
                &mut signal(0),
            );
            assert_eq!(clock.0, 5000);
            assert_eq!(transport.stops, 1);
            assert_eq!(outcome.report.terminal_received, Some(false));
            assert!(outcome.report.has_reason(StopReason::TerminalTimeout));
            let bytes = outcome.archive.unwrap().unwrap();
            assert!(!audit_archive(&bytes).unwrap().terminal_received());
            assert_eq!(
                decode_archive(&bytes)
                    .unwrap()
                    .frames()
                    .last()
                    .unwrap()
                    .active_contexts(),
                1
            );
        }
        #[test]
        fn c11_frame_limit_preserves_cleanup_reserve_without_truncation() {
            struct StaticClock;
            impl CaptureClock for StaticClock {
                fn elapsed_ms(&self) -> u64 {
                    0
                }
                fn wait_ms(&mut self, _: u32) {}
            }
            struct Empty;
            impl ProbeTransport for Empty {
                fn exchange(&mut self, request: Request) -> Result<Vec<u8>, TransportError> {
                    let state = if request.command() == Command::Stop {
                        3
                    } else {
                        1
                    };
                    Ok(empty(request.command(), request.call_id(), state, 0))
                }
            }
            let outcome =
                collect_controlled(|| Ok(Empty), &mut StaticClock, session(), 1, &mut signal(0));
            assert_eq!(outcome.report.first_reason, Some(StopReason::Limit));
            let bytes = outcome.archive.unwrap().unwrap();
            assert_eq!(
                decode_archive(&bytes).unwrap().frames().len(),
                cache_probe_core::MAX_FRAMES - 1
            );
            assert!(audit_archive(&bytes).unwrap().terminal_received());
        }
        #[test]
        fn c11_recovery_waits_for_both_active_and_queued_then_terminal() {
            let mut queued = empty(Command::Drain, 3, 2, 0);
            put32(&mut queued, 68, 1);
            put64(&mut queued, 80, 1);
            let mut final_drain = literal()[208..352].to_vec();
            put32(&mut final_drain, 16, 240);
            put32(&mut final_drain, 24, 1);
            put64(&mut final_drain, 48, 4);
            put32(&mut final_drain, 56, 3);
            put64(&mut final_drain, 80, 1);
            put64(&mut final_drain, 96, 1);
            final_drain.extend_from_slice(&literal()[352..448]);
            let mut script = scripted(vec![
                begin(),
                (Command::Stop, 2, 0, Ok(empty(Command::Stop, 2, 2, 1))),
                (Command::Drain, 3, 16, Ok(queued)),
                (Command::Drain, 4, 16, Ok(final_drain)),
            ]);
            let outcome = capture_script(&mut script, &mut signal(0), 1, 1);
            assert_eq!(outcome.report.terminal_received, Some(true));
            assert_eq!(
                decode_archive(&outcome.archive.unwrap().unwrap())
                    .unwrap()
                    .frames()
                    .len(),
                4
            );
            assert!(script.rows.is_empty());
        }
        #[test]
        fn c12_abort_empty_keeps_v1_footer_and_unknown_external_facts() {
            let mut script = scripted(vec![begin(), stop(2)]);
            let outcome = capture_script(&mut script, &mut signal(0), 1, 1);
            let bytes = outcome.archive.unwrap().unwrap();
            let report = audit_archive(&bytes).unwrap();
            assert!(report.terminal_received());
            assert!(!report.retained_records_reconciled());
            assert!(report.to_json().contains("EMPTY"));
            let footer = &bytes[bytes.len() - 64..];
            assert_eq!(&footer[24..28], &[0; 4]);
            assert_eq!(&footer[52..56], &[0; 4]);
            let diagnostic = outcome.report.to_json();
            assert!(diagnostic.contains("\"saved\":null"));
            assert!(diagnostic.contains("\"process_exit\":null"));
            assert!(bytes.starts_with(b"WCHP04AR"));
        }
        #[test]
        fn c12_terminal_response_does_not_accept_late_signal_or_issue_more_commands() {
            let mut script = scripted(vec![
                begin(),
                (Command::Drain, 2, 16, Ok(empty(Command::Drain, 2, 1, 0))),
                stop(3),
            ]);
            let outcome = capture_script(&mut script, &mut signal(0), 3, 1);
            assert_eq!(outcome.report.first_reason, Some(StopReason::Duration));
            assert!(!outcome.report.abort_accepted);
            assert!(outcome.archive.unwrap().is_some());
            assert!(script.rows.is_empty());
        }
    }
}
