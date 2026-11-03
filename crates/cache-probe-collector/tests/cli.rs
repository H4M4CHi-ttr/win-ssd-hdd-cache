use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Input(PathBuf);
impl Input {
    fn new(bytes: &[u8]) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "p04-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("ordinary.native");
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
        fs::remove_dir(self.0.parent().unwrap()).unwrap();
    }
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cache-probe-collector"))
        .args(args)
        .output()
        .unwrap()
}
fn fixed(output: Output, exit: i32, code: &str) {
    assert_eq!(output.status.code(), Some(exit));
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, format!("{code}\n").as_bytes());
}
fn bytes() -> Vec<u8> {
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

#[test]
fn usage_never_collects() {
    for args in [
        vec![],
        vec!["collect"],
        vec!["audit"],
        vec!["project", "--input"],
        vec!["audit", "--input", "-", "--unknown"],
        vec!["--help", "extra"],
        vec!["collect", "--experimental-live", "--duration-ms", "01"],
        vec!["collect", "--experimental-live", "--duration-ms", "0"],
        vec!["collect", "--experimental-live", "--duration-ms", "30001"],
    ] {
        fixed(run(&args), 2, "PROBE_USAGE");
    }
    let help = run(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(help.stderr.is_empty());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("unqualified")
    );
}

#[cfg(any(not(windows), not(feature = "experimental-live")))]
#[test]
fn live_disabled_before_platform_calls() {
    fixed(
        run(&["collect", "--experimental-live", "--duration-ms", "1"]),
        2,
        "PROBE_LIVE_DISABLED",
    );
}

#[test]
fn audit_and_project_exit4() {
    let input = Input::new(&bytes());
    for (command, expected) in [
        (
            "audit",
            include_str!("../../../tests/fixtures/capture-probe-v1/report-read-pair.json"),
        ),
        (
            "project",
            include_str!("../../../tests/fixtures/capture-probe-v1/project-read-pair.json"),
        ),
    ] {
        let out = run(&[command, "--input", input.0.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(4));
        assert_eq!(out.stdout, expected.as_bytes());
        assert!(out.stderr.is_empty());
    }
}

#[test]
fn redacted_input_and_limits() {
    fixed(
        run(&[
            "audit",
            "--input",
            "/definitely-absent-sensitive-person-name.native",
        ]),
        3,
        "PROBE_INPUT_IO",
    );
    for (bytes, error) in [
        (vec![0xff; 17], "INVALID_PROBE_ARCHIVE"),
        (vec![0; 1_048_576], "INVALID_PROBE_ARCHIVE"),
        (vec![0; 1_048_577], "PROBE_TOO_LARGE"),
    ] {
        let input = Input::new(&bytes);
        fixed(
            run(&["audit", "--input", input.0.to_str().unwrap()]),
            2,
            error,
        );
    }
    let input = Input::new(&[]);
    fixed(
        run(&[
            "project",
            "--input",
            input.0.parent().unwrap().to_str().unwrap(),
        ]),
        3,
        "PROBE_INPUT_IO",
    );
}

#[test]
fn duplicate_options_rejected() {
    fixed(
        run(&["audit", "--input", "secret1", "--input", "secret2"]),
        2,
        "PROBE_USAGE",
    );
    fixed(
        run(&[
            "collect",
            "--experimental-live",
            "--experimental-live",
            "--duration-ms",
            "1",
        ]),
        2,
        "PROBE_USAGE",
    );
    fixed(
        run(&[
            "collect",
            "--experimental-live",
            "--duration-ms",
            "1",
            "--duration-ms",
            "1",
        ]),
        2,
        "PROBE_USAGE",
    );
    fixed(
        run(&[
            "collect",
            "--experimental-live",
            "--duration-ms",
            "1",
            "--output",
            "secret",
        ]),
        2,
        "PROBE_USAGE",
    );
}

#[cfg(windows)]
#[test]
fn windows_forbidden_namespace_is_rejected_before_open() {
    for path in [
        r"\\.\PhysicalDrive0",
        r"\\.\pipe\anything",
        r"\\?\GLOBALROOT\Device\Harddisk0",
        r"\\server\share\input.native",
        r"\\?\UNC\server\share\input.native",
        r"C:\NUL.native",
        "nul",
        "COM1",
        "LPT²",
    ] {
        fixed(run(&["audit", "--input", path]), 3, "PROBE_INPUT_IO");
    }
}
