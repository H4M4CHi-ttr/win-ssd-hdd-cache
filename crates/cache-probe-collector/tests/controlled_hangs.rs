//! Separate-process synchronous hangs. Only fake storage transport is used.
//! stderr is the synthetic binary output pipe because Rust's test harness owns
//! stdout; no diagnostic is written to this pipe on a passing child path.
use cache_probe_collector::capture::{
    AbortPollError, AbortSignal, CaptureClock, CollectError, ProbeTransport, TransportError,
    collect_controlled, emit_archive,
};
use cache_probe_core::{Command, Request, audit_archive, decode_archive};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command as Process, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
#[path = "../../cache-probe-launcher/src/supervision.rs"]
mod supervision;
use supervision::{Classification, Monitor, MonitorError, ProcessObservation};

const CHILD_TEST: &str = "synthetic_hang_child";
const WATCHDOG: Duration = Duration::from_secs(20);
static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    Open,
    Begin,
    Drain,
    PoppedDrain,
    Stop,
    RecoveryDrain,
    RecoveryStatus,
    Write,
    Flush,
}
impl Point {
    fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Begin => "begin",
            Self::Drain => "drain",
            Self::PoppedDrain => "popped",
            Self::Stop => "stop",
            Self::RecoveryDrain => "recovery-drain",
            Self::RecoveryStatus => "status",
            Self::Write => "write",
            Self::Flush => "flush",
        }
    }
    fn parse(name: &str) -> Self {
        [
            Self::Open,
            Self::Begin,
            Self::Drain,
            Self::PoppedDrain,
            Self::Stop,
            Self::RecoveryDrain,
            Self::RecoveryStatus,
            Self::Write,
            Self::Flush,
        ]
        .into_iter()
        .find(|point| point.name() == name)
        .expect("unknown synthetic point")
    }
}

fn log(dir: &Path, text: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("ledger"))
        .unwrap();
    writeln!(file, "{text}").unwrap();
    file.flush().unwrap();
}
fn barrier(dir: &Path, point: Point) -> bool {
    log(dir, &format!("Entered:{}", point.name()));
    fs::write(dir.join("entered"), point.name()).unwrap();
    let mut release = [0];
    io::stdin()
        .read_exact(&mut release)
        .expect("supervisor must explicitly Release");
    assert!(matches!(release[0], b'o' | b'e'));
    log(
        dir,
        if release[0] == b'o' {
            "Released:ok"
        } else {
            "Released:error"
        },
    );
    release[0] == b'o'
}

struct Signal {
    dir: PathBuf,
    accepted: bool,
}
impl AbortSignal for Signal {
    fn poll(&mut self) -> Result<bool, AbortPollError> {
        let published = self.dir.join("abort").exists();
        if published && !self.accepted {
            log(&self.dir, "Accepted");
            self.accepted = true;
        }
        Ok(published)
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
fn literal() -> Vec<u8> {
    let hex: String = include_str!("../../../tests/fixtures/capture-probe-v1/read-pair.hex")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn header(req: Request, state: u32, active: u32, queued: u32, total: u64) -> Vec<u8> {
    let mut bytes = literal()[64..208].to_vec();
    put32(&mut bytes, 28, req.command() as u32);
    put64(&mut bytes, 48, req.call_id());
    put32(&mut bytes, 56, state);
    put32(&mut bytes, 64, active);
    put32(&mut bytes, 68, queued);
    put64(&mut bytes, 80, total + u64::from(queued));
    put64(&mut bytes, 96, total);
    put32(&mut bytes, 112, req.maximum());
    bytes
}
fn records(req: Request, first: u64, count: u32, state: u32) -> Vec<u8> {
    let mut bytes = header(req, state, 0, 0, first + u64::from(count));
    put32(&mut bytes, 16, 144 + 96 * count);
    put32(&mut bytes, 24, count);
    for offset in (0..u64::from(count)).step_by(2) {
        let mut pair = literal()[352..544].to_vec();
        let seq = first + offset;
        put64(&mut pair, 0, seq);
        put64(&mut pair, 8, seq / 2 + 1);
        put64(&mut pair, 96, seq + 1);
        put64(&mut pair, 104, seq / 2 + 1);
        bytes.extend(pair);
    }
    bytes
}
struct Fake {
    dir: PathBuf,
    point: Point,
    calls: u64,
    popped: u64,
    stops: u64,
}
impl Drop for Fake {
    fn drop(&mut self) {
        log(&self.dir, "Close");
    }
}
impl ProbeTransport for Fake {
    fn exchange(&mut self, req: Request) -> Result<Vec<u8>, TransportError> {
        self.calls += 1;
        assert_eq!(req.call_id(), self.calls);
        assert_eq!(req.session_id(), 7);
        let letter = match req.command() {
            Command::Begin => 'B',
            Command::Drain => 'D',
            Command::Stop => 'S',
            Command::Status => 'T',
        };
        assert_eq!(
            req.maximum(),
            if req.command() == Command::Drain {
                16
            } else {
                0
            }
        );
        log(&self.dir, &format!("{letter}{}", req.call_id()));
        match req.command() {
            Command::Begin => {
                if self.point == Point::Begin && !barrier(&self.dir, self.point) {
                    return Err(TransportError);
                }
                Ok(header(req, 1, 0, 0, 0))
            }
            Command::Drain => {
                if self.point == Point::RecoveryStatus {
                    let bytes = records(req, self.popped, 16, 1);
                    self.popped += 16;
                    return Ok(bytes);
                }
                if req.call_id() == 2 && self.point == Point::PoppedDrain {
                    self.popped = 2;
                    log(&self.dir, "Popped:2");
                    if !barrier(&self.dir, self.point) {
                        return Err(TransportError);
                    }
                    return Ok(records(req, 0, 2, 1));
                }
                let blocked = (req.call_id() == 2 && self.point == Point::Drain)
                    || (req.call_id() == 4 && self.point == Point::RecoveryDrain);
                if blocked && !barrier(&self.dir, self.point) {
                    return Err(TransportError);
                }
                Ok(header(
                    req,
                    if self.stops > 0 { 3 } else { 1 },
                    0,
                    0,
                    self.popped,
                ))
            }
            Command::Stop => {
                self.stops += 1;
                if self.point == Point::Stop && self.stops == 1 && !barrier(&self.dir, self.point) {
                    return Err(TransportError);
                }
                if self.point == Point::RecoveryDrain && self.stops == 1 {
                    return Ok(header(req, 2, 1, 0, 0));
                }
                if self.point == Point::RecoveryStatus {
                    return Ok(header(
                        req,
                        if self.stops == 1 { 2 } else { 3 },
                        if self.stops == 1 { 1 } else { 0 },
                        if self.stops == 1 { 1 } else { 2 },
                        4096,
                    ));
                }
                Ok(header(req, 3, 0, 0, self.popped))
            }
            Command::Status => {
                assert_eq!(self.point, Point::RecoveryStatus);
                assert_eq!(req.call_id(), 259);
                if !barrier(&self.dir, self.point) {
                    return Err(TransportError);
                }
                Ok(header(req, 3, 0, 2, 4096))
            }
        }
    }
}
struct Output {
    dir: PathBuf,
    point: Point,
    inner: io::Stderr,
    write_count: u32,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_count += 1;
        assert_eq!(self.write_count, 1);
        log(&self.dir, "Write");
        if self.point == Point::Write && !barrier(&self.dir, self.point) {
            return Err(io::Error::other("synthetic released write failure"));
        }
        self.inner.write_all(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        log(&self.dir, "Flush");
        if self.point == Point::Flush && !barrier(&self.dir, self.point) {
            return Err(io::Error::other("synthetic released flush failure"));
        }
        self.inner.flush()
    }
}

/// The only worker entry; ordinary test runs execute no worker without this
/// private synthetic environment, and it never creates a native transport.
#[test]
fn synthetic_hang_child() {
    let Some(dir) = std::env::var_os("P04_SYNTHETIC_HANG_DIR") else {
        return;
    };
    let dir = PathBuf::from(dir);
    let point = Point::parse(&std::env::var("P04_SYNTHETIC_HANG_POINT").unwrap());
    let mut signal = Signal {
        dir: dir.clone(),
        accepted: false,
    };
    let mut clock = Clock(0);
    let outcome = collect_controlled(
        || {
            log(&dir, "Open");
            if point == Point::Open && !barrier(&dir, point) {
                return Err(TransportError);
            }
            Ok(Fake {
                dir: dir.clone(),
                point,
                calls: 0,
                popped: 0,
                stops: 0,
            })
        },
        &mut clock,
        NonZeroU64::new(7).unwrap(),
        if point == Point::RecoveryStatus {
            30_000
        } else {
            1
        },
        &mut signal,
    );
    fs::write(dir.join("report"), outcome.report.to_json()).unwrap();
    match outcome.archive {
        Ok(Some(bytes)) => {
            fs::write(dir.join("expected-output"), &bytes).unwrap();
            let result = emit_archive(
                &mut Output {
                    dir: dir.clone(),
                    point,
                    inner: io::stderr(),
                    write_count: 0,
                },
                &bytes,
            );
            fs::write(
                dir.join("result"),
                if result.is_ok() {
                    "archive-emitted"
                } else {
                    "output-error"
                },
            )
            .unwrap();
        }
        Ok(None) => fs::write(dir.join("result"), "abort-before-begin").unwrap(),
        Err(CollectError::DeviceUnavailable) => {
            fs::write(dir.join("result"), "device-unavailable").unwrap()
        }
        Err(error) => panic!("unexpected synthetic collection result: {}", error.code()),
    }
    log(&dir, "ChildDone");
}

struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    dir: PathBuf,
    root: PathBuf,
    received: Arc<Mutex<Vec<u8>>>,
    reader: Option<thread::JoinHandle<io::Result<()>>>,
}
impl Worker {
    fn spawn(point: Point) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let dir = root.join(format!(
            "p04-synthetic-hang-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let mut child = Process::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
            .env("P04_SYNTHETIC_HANG_DIR", &dir)
            .env("P04_SYNTHETIC_HANG_POINT", point.name())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let mut pipe = child.stderr.take().unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink = received.clone();
        let reader = thread::spawn(move || {
            let mut chunk = [0; 8192];
            loop {
                let count = pipe.read(&mut chunk)?;
                if count == 0 {
                    return Ok(());
                }
                sink.lock().unwrap().extend_from_slice(&chunk[..count]);
            }
        });
        Self {
            child,
            input,
            dir,
            root,
            received,
            reader: Some(reader),
        }
    }
    fn wait_entered(&mut self, point: Point) {
        let start = Instant::now();
        while !self.dir.join("entered").exists() {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "child exited before Entered; binary pipe bytes={}",
                self.received.lock().unwrap().len()
            );
            assert!(
                start.elapsed() < WATCHDOG,
                "harness timed out before Entered"
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            fs::read_to_string(self.dir.join("entered")).unwrap(),
            point.name()
        );
    }
    fn ledger(&self) -> Vec<String> {
        fs::read_to_string(self.dir.join("ledger"))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
    fn release(&mut self, success: bool) {
        self.input
            .as_mut()
            .unwrap()
            .write_all(if success { b"o" } else { b"e" })
            .unwrap();
        self.input.take();
    }
    fn reap(&mut self) {
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "synthetic child failed; bytes={}",
                    self.received.lock().unwrap().len()
                );
                break;
            }
            assert!(
                start.elapsed() < WATCHDOG,
                "harness timed out after Release"
            );
            thread::sleep(Duration::from_millis(2));
        }
        self.reader.take().unwrap().join().unwrap().unwrap();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(mut input) = self.input.take() {
            let _ = input.write_all(b"e");
        }
        // Error cleanup is restricted to this fake child. Always reap it; a kill
        // never counts as successful cooperative completion of a test case.
        let until = Instant::now() + Duration::from_secs(2);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < until {
            thread::sleep(Duration::from_millis(2));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Ok(resolved) = self.dir.canonicalize()
            && resolved.parent() == Some(self.root.as_path())
            && resolved.file_name() == self.dir.file_name()
        {
            let _ = fs::remove_dir_all(resolved);
        }
    }
}

fn operations(ledger: &[String]) -> Vec<&str> {
    ledger
        .iter()
        .filter_map(|line| {
            if line == "Open"
                || line == "Close"
                || (line.len() > 1
                    && matches!(line.as_bytes()[0], b'B' | b'D' | b'S' | b'T')
                    && line[1..].bytes().all(|b| b.is_ascii_digit()))
            {
                Some(line.as_str())
            } else {
                None
            }
        })
        .collect()
}
fn prefix(point: Point) -> Vec<String> {
    if point == Point::RecoveryStatus {
        let mut expected = vec!["Open".into(), "B1".into()];
        expected.extend((2..=257).map(|call| format!("D{call}")));
        expected.extend(["S258".into(), "T259".into()]);
        return expected;
    }
    match point {
        Point::Open => vec!["Open"],
        Point::Begin => vec!["Open", "B1"],
        Point::Drain | Point::PoppedDrain => vec!["Open", "B1", "D2"],
        Point::Stop => vec!["Open", "B1", "D2", "S3"],
        Point::RecoveryDrain => vec!["Open", "B1", "D2", "S3", "D4"],
        Point::Write | Point::Flush => vec!["Open", "B1", "D2", "S3", "Close"],
        Point::RecoveryStatus => unreachable!(),
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}
fn expected(point: Point, success: bool) -> Vec<String> {
    let mut trace = prefix(point);
    match point {
        Point::Open => {
            if success {
                trace.push("Close".into());
            }
        }
        Point::Begin => {
            if success {
                trace.push("S2".into());
            }
            trace.push("Close".into());
        }
        Point::Drain | Point::PoppedDrain => trace.extend(["S3".into(), "Close".into()]),
        Point::Stop => {
            if !success {
                trace.push("S4".into());
            }
            trace.push("Close".into());
        }
        Point::RecoveryDrain => {
            if !success {
                trace.push("S5".into());
            }
            trace.push("Close".into());
        }
        Point::RecoveryStatus => {
            if !success {
                trace.push("S260".into());
            }
            trace.push("Close".into());
        }
        Point::Write | Point::Flush => {}
    }
    trace
}
fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}
fn digest_hex(bytes: &[u8]) -> String {
    fingerprint(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn run(point: Point, success: bool) {
    let mut worker = Worker::spawn(point);
    worker.wait_entered(point);
    assert_eq!(operations(&worker.ledger()), prefix(point));
    if point == Point::PoppedDrain {
        assert!(worker.ledger().contains(&"Popped:2".into()));
    }
    if !matches!(point, Point::Write | Point::Flush) {
        assert!(!worker.dir.join("report").exists());
    }
    let before_output = if matches!(point, Point::Write | Point::Flush) {
        Some(fs::read(worker.dir.join("expected-output")).unwrap())
    } else {
        None
    };
    let before_report = if matches!(point, Point::Write | Point::Flush) {
        Some(fs::read(worker.dir.join("report")).unwrap())
    } else {
        None
    };
    if point == Point::Flush {
        let expected = fs::read(worker.dir.join("expected-output")).unwrap();
        let start = Instant::now();
        while worker.received.lock().unwrap().len() < expected.len() {
            assert!(
                start.elapsed() < WATCHDOG,
                "pipe reader did not receive pre-flush bytes"
            );
            thread::yield_now();
        }
        assert_eq!(*worker.received.lock().unwrap(), expected);
    } else {
        assert!(worker.received.lock().unwrap().is_empty());
    }
    assert!(!worker.ledger().contains(&"Accepted".into()));
    let mut monitor = Monitor::new(15_000).unwrap();
    // The external clock advances independently while the worker is blocked in
    // a real stdin read. No worker clock/poll/exchange can make this progress.
    fs::write(worker.dir.join("abort"), b"publish").unwrap();
    monitor.request_published(100).unwrap();
    let pending = ProcessObservation {
        received_bytes: worker.received.lock().unwrap().len() as u64,
        ..Default::default()
    };
    monitor.observe(14_999, pending).unwrap();
    assert_eq!(monitor.snapshot().classification(), Classification::Pending);
    assert!(worker.child.try_wait().unwrap().is_none());
    assert_eq!(operations(&worker.ledger()), prefix(point));
    monitor.observe(15_000, pending).unwrap();
    let deadline = monitor.snapshot();
    assert_eq!(
        deadline.classification(),
        Classification::DeadlineUnconfirmed
    );
    assert_eq!(deadline.deadline_unconfirmed_at, Some(15_000));
    assert_eq!(deadline.process_exited_at, None);
    assert_eq!(deadline.output_complete_at, None);
    assert_eq!(deadline.facts.saved, None);
    assert!(!worker.dir.join("result").exists());
    assert!(!worker.ledger().contains(&"Accepted".into()));
    worker.release(success);
    worker.reap();
    assert_eq!(operations(&worker.ledger()), expected(point, success));
    let report = fs::read_to_string(worker.dir.join("report")).unwrap();
    if let Some(before) = before_report {
        assert_eq!(report.as_bytes(), before);
    }
    let result = fs::read_to_string(worker.dir.join("result")).unwrap();
    let received = worker.received.lock().unwrap().clone();
    assert!(report.contains(
        if point == Point::Open || (point == Point::Begin && !success) {
            "\"stop_acknowledged\":null"
        } else {
            "\"stop_acknowledged\":true"
        }
    ));
    if point == Point::Open {
        assert!(report.contains("\"begin_submitted\":false"));
        assert!(report.contains("\"begin_acknowledged\":null"));
    } else {
        assert!(report.contains("\"begin_submitted\":true"));
        assert!(report.contains(if point == Point::Begin && !success {
            "\"begin_acknowledged\":null"
        } else {
            "\"begin_acknowledged\":true"
        }));
    }
    let accepted = match point {
        Point::Open => success,
        Point::Begin | Point::Drain | Point::PoppedDrain | Point::RecoveryStatus => true,
        Point::Stop | Point::RecoveryDrain => !success,
        Point::Write | Point::Flush => false,
    };
    assert!(report.contains(if accepted {
        "\"abort_accepted\":true"
    } else {
        "\"abort_accepted\":false"
    }));
    let final_ledger = worker.ledger();
    if let Some(accepted_at) = final_ledger.iter().position(|line| line == "Accepted") {
        assert!(
            operations(&final_ledger[accepted_at + 1..])
                .iter()
                .all(|op| !op.starts_with('D')),
            "normal/recovery DRAIN after acceptance"
        );
    }
    if point == Point::Open || (point == Point::Begin && !success) {
        assert!(received.is_empty());
        assert!(!report.contains("\"stop_acknowledged\":true"));
        assert!(!report.contains("\"terminal_received\":true"));
        assert_eq!(
            result,
            if point == Point::Open && success {
                "abort-before-begin"
            } else {
                "device-unavailable"
            }
        );
    } else {
        let archive = fs::read(worker.dir.join("expected-output")).unwrap();
        if let Some(before) = before_output {
            assert_eq!(archive, before);
        }
        let decoded = decode_archive(&archive).unwrap();
        let audit = audit_archive(&archive).unwrap();
        if matches!(point, Point::Write | Point::Flush) && !success {
            assert_eq!(result, "output-error");
        } else {
            assert_eq!(result, "archive-emitted");
        }
        if point == Point::Write && !success {
            assert!(received.is_empty());
        } else {
            assert_eq!(received, archive);
        }
        if point == Point::RecoveryStatus {
            assert!(!audit.terminal_received());
            assert_eq!(decoded.record_count(), 4096);
        }
        if point == Point::PoppedDrain {
            assert_eq!(decoded.record_count(), if success { 2 } else { 0 });
        }
        if !success && !matches!(point, Point::Write | Point::Flush) {
            assert!(!audit.retained_records_reconciled());
        }
        if point == Point::Flush && !success {
            assert_eq!(received, archive);
            assert!(audit.terminal_received());
        }
    }
    // Late completion adds facts at a later timestamp, never rewrites the
    // 15000 observation, never claims saved, and cannot erase monitor loss.
    monitor
        .observe(
            15_001,
            ProcessObservation {
                exited: true,
                stdout_eof: true,
                received_bytes: received.len() as u64,
                received_hash: Some(fingerprint(&received)),
                forwarding_complete: true,
                saved: None,
            },
        )
        .unwrap();
    let late = monitor.snapshot();
    assert_eq!(late.deadline_unconfirmed_at, Some(15_000));
    assert_eq!(late.process_exited_at, Some(15_001));
    assert_eq!(late.output_complete_at, Some(15_001));
    assert_eq!(late.classification(), Classification::DeadlineUnconfirmed);
    assert_eq!(late.facts.saved, None);
    assert_eq!(deadline.process_exited_at, None);
    println!(
        "{{\"schema\":\"p04-hang-case-v1\",\"point\":\"{}\",\"release\":\"{}\",\"entered\":true,\"prefix\":{:?},\"actual\":{:?},\"deadlineObservedAt\":15000,\"deadlineSticky\":true,\"lateProcessExitAt\":15001,\"receivedBytes\":{},\"receivedSha256\":\"{}\",\"result\":\"{}\",\"report\":{},\"realDeviceAccess\":false,\"driverInstalledOrLoaded\":false,\"registeredOrAttached\":false,\"runtimeQualified\":false,\"windowsClientQualified\":false}}",
        point.name(),
        if success { "ok" } else { "error" },
        prefix(point),
        operations(&final_ledger),
        received.len(),
        digest_hex(&received),
        result,
        report.trim()
    );
}

#[test]
fn h01_open_success_and_failure_after_release() {
    for success in [true, false] {
        run(Point::Open, success);
    }
}
#[test]
fn h02_begin_success_and_failure_after_release() {
    for success in [true, false] {
        run(Point::Begin, success);
    }
}
#[test]
fn h03_drain_success_and_failure_after_release() {
    for success in [true, false] {
        run(Point::Drain, success);
    }
}
#[test]
fn h03_already_popped_drain_success_and_failure() {
    for success in [true, false] {
        run(Point::PoppedDrain, success);
    }
}
#[test]
fn h04_stop_success_and_failure_after_release() {
    for success in [true, false] {
        run(Point::Stop, success);
    }
}
#[test]
fn h04_recovery_drain_success_and_failure() {
    for success in [true, false] {
        run(Point::RecoveryDrain, success);
    }
}
#[test]
fn h04_budget_status_success_and_failure() {
    for success in [true, false] {
        run(Point::RecoveryStatus, success);
    }
}
#[test]
fn h05_write_success_and_failure_after_release() {
    for success in [true, false] {
        run(Point::Write, success);
    }
}
#[test]
fn h05_flush_success_and_failure_after_release() {
    for success in [true, false] {
        run(Point::Flush, success);
    }
}

#[test]
fn h06_monitor_loss_regression_and_completed_receiver_are_independent() {
    assert_eq!(
        digest_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        digest_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let mut delayed = Monitor::new(15_000).unwrap();
    delayed
        .observe(14_999, ProcessObservation::default())
        .unwrap();
    delayed.note_request_published(100, 14_999).unwrap();
    assert_eq!(delayed.snapshot().request_published_at, Some(100));
    assert_eq!(delayed.snapshot().observed_at, 14_999);
    assert_eq!(
        delayed.note_request_published(15_000, 14_999),
        Err(MonitorError::InvalidEvidence)
    );
    let mut monitor = Monitor::new(15_000).unwrap();
    monitor.request_published(10).unwrap();
    monitor
        .observe(14_999, ProcessObservation::default())
        .unwrap();
    monitor.lose_monitor(14_999).unwrap();
    assert_eq!(
        monitor.snapshot().classification(),
        Classification::MonitorLost
    );
    assert_eq!(monitor.snapshot().process_exited_at, None);
    monitor
        .observe(15_000, ProcessObservation::default())
        .unwrap();
    assert_eq!(
        monitor.snapshot().classification(),
        Classification::MonitorLost
    );
    assert_eq!(
        monitor.observe(14_999, ProcessObservation::default()),
        Err(MonitorError::ClockRegressed)
    );
    let completed = ProcessObservation {
        exited: true,
        stdout_eof: true,
        received_bytes: 3,
        received_hash: Some(fingerprint(b"abc")),
        forwarding_complete: true,
        saved: None,
    };
    monitor.observe(15_001, completed).unwrap();
    assert_eq!(
        monitor.snapshot().classification(),
        Classification::MonitorLost
    );
    assert_eq!(monitor.snapshot().deadline_unconfirmed_at, Some(15_000));
    assert_eq!(
        monitor.observe(15_002, ProcessObservation::default()),
        Err(MonitorError::FactRegressed)
    );
    let mut fresh = Monitor::new(15_000).unwrap();
    fresh.observe(5, completed).unwrap();
    assert_eq!(fresh.snapshot().classification(), Classification::Completed);
    assert_eq!(fresh.snapshot().save_confirmed_at, None);
    assert_eq!(
        fresh.observe(
            6,
            ProcessObservation {
                received_bytes: 4,
                ..completed
            }
        ),
        Err(MonitorError::FactRegressed)
    );
    let mut impossible = Monitor::new(15_000).unwrap();
    assert_eq!(
        impossible.observe(
            1,
            ProcessObservation {
                saved: Some(true),
                ..Default::default()
            }
        ),
        Err(MonitorError::InvalidEvidence)
    );
    assert_eq!(
        impossible.observe(
            1,
            ProcessObservation {
                forwarding_complete: true,
                ..Default::default()
            }
        ),
        Err(MonitorError::InvalidEvidence)
    );
    assert_eq!(
        impossible.observe(
            1,
            ProcessObservation {
                stdout_eof: true,
                received_hash: Some([0; 32]),
                saved: Some(true),
                ..Default::default()
            }
        ),
        Err(MonitorError::InvalidEvidence)
    );
}
