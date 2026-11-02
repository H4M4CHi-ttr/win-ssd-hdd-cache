use serde_json::{Value, json};
use xtask::validate_host_workflow;

const WORKFLOW: &str = include_str!("../../.github/workflows/host-tests.yml");
fn fixture() -> Value {
    serde_json::from_str(WORKFLOW).unwrap()
}
fn rejected(value: &Value) {
    assert!(
        validate_host_workflow(&value.to_string()).is_err(),
        "{value}"
    );
}

#[test]
fn actual_workflow_passes_with_exact_toolchain_and_lf_fixtures() {
    assert_eq!(validate_host_workflow(WORKFLOW), Ok(()));
    assert_eq!(
        include_str!("../../rust-toolchain.toml"),
        "[toolchain]\nchannel = \"1.99.1\"\nprofile = \"minimal\"\ncomponents = [\"rustfmt\", \"clippy\"]\n"
    );
    assert_eq!(include_str!("../../.gitattributes"), "* text=auto eol=lf\n");
}
#[test]
fn triggers_permissions_and_credentials_cannot_be_expanded() {
    for event in [
        "pull_request_target",
        "workflow_run",
        "workflow_call",
        "workflow_dispatch",
    ] {
        let mut v = fixture();
        v["on"][event] = json!({});
        rejected(&v);
    }
    for value in [
        json!("write-all"),
        json!({"contents":"write"}),
        json!({"contents":"read","id-token":"write"}),
    ] {
        let mut v = fixture();
        v["permissions"] = value;
        rejected(&v);
    }
    for (field, value) in [
        ("token", json!("${{ secrets.GH_PAT }}")),
        ("ssh-key", json!("${{ secrets.KEY }}")),
        ("persist-credentials", json!(true)),
        ("allow-unsafe-pr-checkout", json!(true)),
    ] {
        let mut v = fixture();
        v["jobs"]["host"]["steps"][0]["with"][field] = value;
        rejected(&v);
    }
}
#[test]
fn runner_matrix_and_job_privilege_are_closed() {
    for os in [
        json!(["ubuntu-24.04"]),
        json!(["ubuntu-24.04", "self-hosted"]),
        json!(["ubuntu-latest", "windows-2025"]),
    ] {
        let mut v = fixture();
        v["jobs"]["host"]["strategy"]["matrix"]["os"] = os;
        rejected(&v);
    }
    for field in [
        "permissions",
        "secrets",
        "environment",
        "container",
        "services",
        "env",
        "uses",
        "if",
        "continue-on-error",
    ] {
        let mut v = fixture();
        v["jobs"]["host"][field] = json!(true);
        rejected(&v);
    }
    let mut v = fixture();
    v["jobs"]["other"] = json!({"runs-on":"self-hosted"});
    rejected(&v);
    let mut v = fixture();
    v["jobs"]["host"]["runs-on"] = json!("self-hosted");
    rejected(&v);
    let mut v = fixture();
    v["jobs"]["host"]["strategy"]["matrix"]["exclude"] = json!([{ "os":"windows-2025" }]);
    rejected(&v);
}
#[test]
fn steps_are_an_exact_allowlist_not_a_dangerous_word_blacklist() {
    for command in [
        "cargo test --locked || true",
        "cargo test",
        "diskpart /s prepare.txt",
        "pnputil /add-driver test.inf /install",
        "signtool sign /a cache.sys",
        "format D:",
        "sudo true",
        "cargo test --locked\necho extra",
    ] {
        let mut v = fixture();
        v["jobs"]["host"]["steps"][6]["run"] = json!(command);
        rejected(&v);
    }
    for uses in [
        "actions/checkout@v7",
        "actions/checkout@0000000000000000000000000000000000000000",
        "./local-action",
    ] {
        let mut v = fixture();
        v["jobs"]["host"]["steps"][0]["uses"] = json!(uses);
        rejected(&v);
    }
    for field in [
        "if",
        "continue-on-error",
        "env",
        "shell",
        "working-directory",
    ] {
        let mut v = fixture();
        v["jobs"]["host"]["steps"][6][field] = json!(true);
        rejected(&v);
    }
    let mut v = fixture();
    v["jobs"]["host"]["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({"run":"echo extra"}));
    rejected(&v);
}
#[test]
fn required_checks_and_nonzero_guard_cannot_be_removed_or_masked() {
    for i in 0..8 {
        let mut v = fixture();
        v["jobs"]["host"]["steps"].as_array_mut().unwrap().remove(i);
        rejected(&v);
    }
    for command in [
        "cargo test --locked -- --list",
        "echo 'fake: test'",
        "cargo test --locked -- --list || true",
    ] {
        let mut v = fixture();
        v["jobs"]["host"]["steps"][5]["run"] = json!(command);
        rejected(&v);
    }
    let mut v = fixture();
    v["jobs"]["host"]["steps"][1]["run"] = json!("rustup update stable");
    rejected(&v);
    let mut v = fixture();
    v["jobs"]["host"]["strategy"]["fail-fast"] = json!(true);
    rejected(&v);
}
#[test]
fn raw_syntax_duplicates_and_resource_bounds_are_strict() {
    for source in [
        String::new(),
        "name: yaml-only".into(),
        format!("{WORKFLOW}{{}}"),
        WORKFLOW.replacen(
            "\"contents\": \"read\"",
            "\"contents\": \"write\", \"contents\": \"read\"",
            1,
        ),
        WORKFLOW.replacen(
            "\"contents\": \"read\"",
            "\"conten\\u0074s\": \"read\", \"contents\": \"read\"",
            1,
        ),
        format!("{}{}", " ".repeat(65_537), WORKFLOW),
    ] {
        assert!(validate_host_workflow(&source).is_err());
    }
    let v = fixture();
    assert_eq!(
        validate_host_workflow(&serde_json::to_string_pretty(&v).unwrap()),
        Ok(())
    );
}

#[test]
fn actual_result_guard_rejects_zero_and_all_ignored_tests() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let workflow = fixture();
    let test_command = workflow["jobs"]["host"]["steps"][6]["run"]
        .as_str()
        .unwrap();
    let guard = test_command
        .lines()
        .find(|line| line.starts_with("grep -E "))
        .expect("actual execution must have its own nonzero-passed guard");
    let stdin_guard = guard
        .strip_suffix(" \"$RUNNER_TEMP/host-test-results.txt\"")
        .unwrap();
    for (output, expected) in [
        (
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
            false,
        ),
        (
            "ignored_only: test\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\n",
            false,
        ),
        (
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
            true,
        ),
        (
            "test result: ok. 0 passed; 0 failed; 1 ignored;\ntest result: ok. 94 passed; 0 failed; 0 ignored;\n",
            true,
        ),
        (
            "test result: FAILED. 1 passed; 1 failed; 0 ignored;\n",
            false,
        ),
        (
            "prefix test result: ok. 1 passed;\ntest result: ok. 0 passed;\n",
            false,
        ),
    ] {
        let mut child = Command::new("bash")
            .args(["-c", stdin_guard])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(output.as_bytes())
            .unwrap();
        assert_eq!(
            child.wait_with_output().unwrap().status.success(),
            expected,
            "{output}"
        );
    }
}

#[test]
fn actual_result_pipeline_cannot_hide_a_failed_test_command() {
    use std::process::Command;
    let workflow = fixture();
    let run = workflow["jobs"]["host"]["steps"][6]["run"]
        .as_str()
        .unwrap();
    assert!(run.starts_with("set -euo pipefail\n"));
    assert!(run.contains("cargo test --locked | tee "));
    // Exercise the actual shell failure-propagation mechanism, without running
    // recursive Cargo or any input-derived command.
    let status = Command::new("bash")
        .args(["-c", "set -euo pipefail\nfalse | cat\nexit 0"])
        .status()
        .unwrap();
    assert!(!status.success());
}
