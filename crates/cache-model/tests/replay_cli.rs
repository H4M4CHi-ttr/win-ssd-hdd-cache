use cache_types::{
    evidence::{Category, Fault, Outcome, ViolationCode, parse_evidence},
    trace::TRACE_MAX_BYTES,
};
use std::{
    ffi::OsStr,
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

const BASELINE: &[u8] = include_bytes!("../../../tests/fixtures/replay/origin-read.json");
const EARLY_ACK: &[u8] = include_bytes!("../../../tests/fixtures/replay/early-ack.json");
const PENDING: &[u8] = include_bytes!("../../../tests/fixtures/replay/pending.json");

struct Input {
    root: PathBuf,
    path: PathBuf,
}
impl Input {
    fn new(bytes: &[u8]) -> Self {
        Self::named(OsStr::new("private path 日本語.json"), bytes)
    }
    fn named(name: &OsStr, bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let root = std::env::temp_dir().join(format!(
                "win-cache-replay-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let path = root.join(name);
                    fs::write(&path, bytes).unwrap();
                    return Self { root, path };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("temporary fixture directory: {error}"),
            }
        }
        panic!("cannot allocate unique fixture directory");
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn run(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cache-replay"))
        .args(args)
        .output()
        .unwrap()
}
fn error(output: Output, code: &str) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, format!("error: {code}\n").as_bytes());
}

#[test]
fn baseline_stdout_is_only_stable_lf_terminated_evidence_without_forged_provenance() {
    let input = Input::new(BASELINE);
    let args = [OsStr::new("--trace"), input.path.as_os_str()];
    let first = run(&args);
    let second = run(&args);
    assert_eq!(first.status.code(), Some(0));
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    let json = String::from_utf8(first.stdout).unwrap();
    assert!(json.ends_with('\n'));
    let report = parse_evidence(&json).unwrap();
    assert_eq!(report.outcome, Outcome::Passed);
    assert_eq!(report.source_revision, None);
    assert_eq!(report.trace_digest, None);
    assert_eq!(report.meaningful_observations(), 4);
    assert_eq!(report.assertions_checked(), 1);
    assert!(!json.contains("4141"));
    assert!(!json.contains("private path"));
    assert!(json.contains("\"kernel_runtime_tested\":false"));
    assert!(json.contains("\"production_qualified\":false"));
}

#[test]
fn pending_and_detected_mutant_have_distinct_exits_not_success() {
    let pending = Input::new(PENDING);
    let output = run(&[OsStr::new("--trace"), pending.path.as_os_str()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    assert_eq!(
        parse_evidence(std::str::from_utf8(&output.stdout).unwrap())
            .unwrap()
            .outcome,
        Outcome::Inconclusive
    );
    let input = Input::new(EARLY_ACK);
    let reference = run(&[OsStr::new("--trace"), input.path.as_os_str()]);
    assert_eq!(reference.status.code(), Some(0));
    let mutant = run(&[
        OsStr::new("--fault"),
        OsStr::new("early_ack"),
        OsStr::new("--trace"),
        input.path.as_os_str(),
    ]);
    assert_eq!(mutant.status.code(), Some(3));
    assert!(mutant.stderr.is_empty());
    let report = parse_evidence(std::str::from_utf8(&mutant.stdout).unwrap()).unwrap();
    assert_eq!(report.fault, Fault::EarlyAck);
    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(
        report.violation.as_ref().unwrap().code,
        ViolationCode::EarlyAck
    );
    assert_eq!(report.events_executed, 3);
    assert_eq!(report.checks[Category::OriginResults.index()].failed, 1);
}

#[test]
fn faults_are_explicit_and_do_not_fabricate_detection_on_unrelated_traces() {
    let input = Input::new(BASELINE);
    for fault in ["none", "stale_fill", "early_ack", "late_slot_reuse"] {
        let output = run(&[
            OsStr::new("--trace"),
            input.path.as_os_str(),
            OsStr::new("--fault"),
            OsStr::new(fault),
        ]);
        assert_eq!(output.status.code(), Some(0));
        let report = parse_evidence(std::str::from_utf8(&output.stdout).unwrap()).unwrap();
        assert_eq!(report.fault.as_str(), fault);
        assert_eq!(report.outcome, Outcome::Passed);
    }
}

#[test]
fn duplicate_unknown_missing_and_noncanonical_options_are_usage_errors() {
    for args in [
        vec![],
        vec!["--trace"],
        vec!["--fault"],
        vec!["--fault", "none"],
        vec!["secret-positional"],
        vec!["--unknown", "private-value"],
        vec!["--trace=a.json"],
        vec!["--trace", "a", "--trace", "b"],
        vec!["--trace", "a", "--fault", "none", "--fault", "none"],
        vec!["--trace", "a", "--fault", "private-invalid-fault"],
    ] {
        error(
            run(&args.iter().map(OsStr::new).collect::<Vec<_>>()),
            "USAGE",
        );
    }
}

#[test]
fn input_errors_have_no_report_paths_payload_or_raw_serde_messages() {
    for (bytes, expected) in [
        // Isolate syntax failure: an unknown object key can fail schema first.
        (b"private-token".as_slice(), "INVALID_JSON"),
        (b"{}".as_slice(), "INVALID_SCHEMA"),
        (b"\xffprivate-token".as_slice(), "INPUT_UTF8"),
    ] {
        let input = Input::new(bytes);
        error(
            run(&[OsStr::new("--trace"), input.path.as_os_str()]),
            expected,
        );
    }
    let input = Input::new(BASELINE);
    error(
        run(&[OsStr::new("--trace"), input.root.as_os_str()]),
        "INPUT_IO",
    );
    error(
        run(&[
            OsStr::new("--trace"),
            input.root.join("secret-missing").as_os_str(),
        ]),
        "INPUT_IO",
    );
    fs::OpenOptions::new()
        .write(true)
        .open(&input.path)
        .unwrap()
        .set_len(TRACE_MAX_BYTES as u64 + 1)
        .unwrap();
    error(
        run(&[OsStr::new("--trace"), input.path.as_os_str()]),
        "INPUT_TOO_LARGE",
    );
}

#[cfg(unix)]
#[test]
fn final_symlinks_and_special_files_are_rejected_without_reading_their_contents() {
    let input = Input::new(BASELINE);
    let link = input.root.join("private-link");
    std::os::unix::fs::symlink(&input.path, &link).unwrap();
    error(run(&[OsStr::new("--trace"), link.as_os_str()]), "INPUT_IO");
    error(
        run(&[OsStr::new("--trace"), OsStr::new("/dev/null")]),
        "INPUT_IO",
    );
}

#[cfg(unix)]
#[test]
fn valid_input_does_not_require_a_utf8_filename() {
    use std::os::unix::ffi::OsStrExt;
    let input = Input::named(OsStr::from_bytes(b"private-\xff.json"), BASELINE);
    let output = run(&[OsStr::new("--trace"), input.path.as_os_str()]);
    assert_eq!(output.status.code(), Some(0));
    assert!(parse_evidence(std::str::from_utf8(&output.stdout).unwrap()).is_ok());
}
