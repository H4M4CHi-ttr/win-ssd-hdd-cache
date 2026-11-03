use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use xtask::{
    ManifestViolation, check_repository, validate_backlog, validate_label_manifest,
    validate_markdown_links,
};
const LABELS: &str = include_str!("../../.github/labels.json");
const BACKLOG: &str = include_str!("../../docs/backlog.json");
fn known() -> BTreeSet<String> {
    serde_json::from_str::<Value>(LABELS).unwrap()["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap().to_owned())
        .collect()
}
fn v(code: &str, path: &str) -> ManifestViolation {
    ManifestViolation {
        code: code.into(),
        path: path.into(),
    }
}
fn task(id: &str, number: u64) -> Value {
    json!({"id":id,"title":"A task","agent":"luna","review":"sol","status":"ready","priority":"p1","type":"task","area":"tooling","owner_decision":false,"owner_approval":"not-required","approval_evidence":[],"dependencies":[],"files":["planned/file.rs"],"output":"A result","acceptance":["A test"],"escalation":"Stop on ambiguity","issue_number":number,"issue_url":format!("https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/{number}"),"completion":"pending"})
}
fn backlog(tasks: Vec<Value>) -> String {
    json!({"schema_version":2,"plan_date":"2026-11-02","tasks":tasks}).to_string()
}
fn has(input: &str, code: &str, path: &str) {
    let errors = validate_backlog(input, &known()).unwrap_err();
    assert!(errors.contains(&v(code, path)), "{errors:?}");
    let mut sorted = errors.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(errors, sorted);
    assert_eq!(validate_backlog(input, &known()).unwrap_err(), errors);
}

#[test]
fn baseline_labels_backlog_and_real_repository_pass_nonzero_checks() {
    assert_eq!(validate_label_manifest(LABELS), Ok(()));
    assert_eq!(validate_backlog(BACKLOG, &known()), Ok(()));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let summary = check_repository(&root).unwrap();
    assert_eq!(summary.labels, 28);
    assert_eq!(summary.tasks, 56);
    assert!(summary.documents > 10);
    assert!(summary.relative_links > 10);
}
#[test]
fn label_semantics_aggregate_in_stable_code_path_order() {
    let input=json!({"schema_version":1,"labels":[{"name":"agent:luna","description":"ok"},{"name":"agent:luna","description":""},{"name":"other:x","description":"a"},{"name":"é","description":"x".repeat(101)}]}).to_string();
    assert_eq!(
        validate_label_manifest(&input),
        Err(vec![
            v("DUPLICATE_NAME", "/labels/1/name"),
            v("INVALID_DESCRIPTION", "/labels/1/description"),
            v("INVALID_DESCRIPTION", "/labels/3/description"),
            v("INVALID_NAME", "/labels/3/name"),
            v("UNKNOWN_PREFIX", "/labels/2/name"),
            v("UNKNOWN_PREFIX", "/labels/3/name")
        ])
    );
}
#[test]
fn label_byte_scalar_bounds_and_existing_grammar_are_not_conflated() {
    let make = |name: &str, description: String| {
        json!({"schema_version":1,"labels":[{"name":name,"description":description}]}).to_string()
    };
    assert_eq!(
        validate_label_manifest(&make(&format!("area:{}", "x".repeat(45)), "あ".repeat(100))),
        Ok(())
    );
    assert!(
        validate_label_manifest(&make(&format!("area:{}", "x".repeat(46)), "ok".into()))
            .unwrap_err()
            .contains(&v("INVALID_NAME", "/labels/0/name"))
    );
    assert_eq!(validate_label_manifest(&make("area:", "ok".into())), Ok(()));
}
#[test]
fn duplicate_unknown_missing_and_wrong_json_shapes_are_structural() {
    for input in [
        r#"{"schema_version":1,"schema_version":1,"labels":[]}"#,
        r#"{"schema_version":1,"schema_vers\u0069on":1,"labels":[]}"#,
    ] {
        assert_eq!(
            validate_label_manifest(input),
            Err(vec![v("INVALID_JSON", "/schema_version")])
        );
    }
    assert_eq!(
        validate_label_manifest(
            r#"{"schema_version":1,"labels":[{"name":"area:x","name":"area:y","description":"x"}]}"#
        ),
        Err(vec![v("INVALID_JSON", "/labels/0/name")])
    );
    assert_eq!(
        validate_label_manifest(
            r#"{"schema_version":1,"labels":[{"name":"area:x","description":"x","a/b~c":1}]}"#
        ),
        Err(vec![v("UNKNOWN_FIELD", "/labels/0/a~1b~0c")])
    );
    assert_eq!(
        validate_label_manifest(r#"{"schema_version":1,"labels":[{"name":"area:x"}]}"#),
        Err(vec![v("INVALID_JSON", "/labels/0/description")])
    );
    for input in ["[]", "true", "null"] {
        assert_eq!(
            validate_label_manifest(input),
            Err(vec![v("INVALID_JSON", "")])
        );
    }
    for token in ["1.0", "1e0", "-0", "true", "\"1\"", "1e400"] {
        assert_eq!(
            validate_label_manifest(&format!("{{\"schema_version\":{token},\"labels\":[]}}")),
            Err(vec![v("INVALID_JSON", "/schema_version")])
        );
    }
    assert_eq!(
        validate_label_manifest(r#"{"schema_version":2,"labels":[]}"#),
        Err(vec![v("UNSUPPORTED_SCHEMA", "/schema_version")])
    );
}
#[test]
fn malformed_oversized_and_deep_json_are_rejected_without_panics() {
    for input in [
        "{".to_owned(),
        format!("{LABELS}{{}}"),
        " ".repeat(1_048_577),
        format!("{}0{}", "[".repeat(65), "]".repeat(65)),
    ] {
        assert_eq!(
            validate_label_manifest(&input),
            Err(vec![v("INVALID_JSON", "")])
        );
    }
    assert_eq!(
        validate_backlog(&" ".repeat(4_194_305), &known()),
        Err(vec![v("INVALID_JSON", "")])
    );
}
#[test]
fn ids_issue_mapping_and_duplicate_dependencies_are_checked() {
    let a = task("A01", 1);
    let mut b = task("A01", 1);
    b["issue_url"] = json!("https://other.example/issues/1");
    let input = backlog(vec![a, b]);
    has(&input, "DUPLICATE_ID", "/tasks/1/id");
    has(&input, "DUPLICATE_ISSUE", "/tasks/1/issue_number");
    has(&input, "INVALID_FIELD", "/tasks/1/issue_url");
    let mut a = task("A01", 1);
    a["dependencies"] = json!(["X01", "X01", "A01"]);
    let input = backlog(vec![a]);
    has(&input, "MISSING_DEPENDENCY", "/tasks/0/dependencies/0");
    has(&input, "DUPLICATE_DEPENDENCY", "/tasks/0/dependencies/1");
    has(&input, "SELF_DEPENDENCY", "/tasks/0/dependencies/2");
}
#[test]
fn cycles_and_unaccepted_dependencies_do_not_become_ready() {
    let mut a = task("A01", 1);
    let mut b = task("B01", 2);
    a["dependencies"] = json!(["B01"]);
    b["dependencies"] = json!(["A01"]);
    has(&backlog(vec![a, b]), "DEPENDENCY_CYCLE", "/tasks");
    let mut a = task("A01", 1);
    a["status"] = json!("cancelled");
    a["completion"] = json!("not-planned");
    let mut b = task("B01", 2);
    b["dependencies"] = json!(["A01"]);
    has(
        &backlog(vec![a.clone(), b.clone()]),
        "UNREADY_DEPENDENCY",
        "/tasks/1/dependencies/0",
    );
    b["status"] = json!("done");
    b["completion"] = json!("accepted");
    has(
        &backlog(vec![a, b]),
        "UNREADY_DEPENDENCY",
        "/tasks/1/dependencies/0",
    );
}
#[test]
fn owner_evidence_is_representation_not_implicit_authority() {
    let mut a = task("A01", 1);
    a["owner_decision"] = json!(true);
    a["owner_approval"] = json!("pending");
    has(
        &backlog(vec![a.clone()]),
        "OWNER_APPROVAL_REQUIRED",
        "/tasks/0/owner_approval",
    );
    a["approval_evidence"] =
        json!(["https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/2#issuecomment-1"]);
    has(
        &backlog(vec![a.clone()]),
        "INVALID_APPROVAL",
        "/tasks/0/owner_approval",
    );
    a["owner_approval"] = json!("approved");
    assert_eq!(
        validate_backlog(&backlog(vec![a.clone()]), &known()),
        Ok(())
    );
    a["approval_evidence"] = json!([]);
    has(
        &backlog(vec![a.clone()]),
        "INVALID_APPROVAL",
        "/tasks/0/owner_approval",
    );
    a["approval_evidence"] = json!(["https://github.com/attacker/repo/issues/2"]);
    has(
        &backlog(vec![a]),
        "INVALID_APPROVAL",
        "/tasks/0/approval_evidence/0",
    );
    let mut a = task("A01", 1);
    a["owner_approval"] = json!("approved");
    has(
        &backlog(vec![a]),
        "INVALID_APPROVAL",
        "/tasks/0/owner_approval",
    );
}
#[test]
fn state_completion_enum_and_label_rules_cannot_be_bypassed() {
    for (status, completion) in [
        ("done", "pending"),
        ("ready", "accepted"),
        ("cancelled", "accepted"),
        ("blocked", "not-planned"),
    ] {
        let mut a = task("A01", 1);
        a["status"] = json!(status);
        a["completion"] = json!(completion);
        has(
            &backlog(vec![a]),
            "INVALID_COMPLETION",
            "/tasks/0/completion",
        );
    }
    let mut a = task("A01", 1);
    a["status"] = json!("needs-decision");
    has(
        &backlog(vec![a]),
        "INVALID_APPROVAL",
        "/tasks/0/owner_approval",
    );
    let mut a = task("A01", 1);
    a["type"] = json!("tracking");
    has(&backlog(vec![a]), "INVALID_FIELD", "/tasks/0/type");
    let mut labels = known();
    labels.remove("agent:luna");
    assert!(
        validate_backlog(&backlog(vec![task("A01", 1)]), &labels)
            .unwrap_err()
            .contains(&v("UNKNOWN_LABEL", "/tasks/0/agent"))
    );
    let mut a = task("A01", 1);
    a["status"] = json!("blocked");
    assert_eq!(validate_backlog(&backlog(vec![a]), &known()), Ok(()));
}
#[test]
fn real_dates_and_planned_paths_are_validated_without_requiring_files() {
    let input = backlog(vec![task("A01", 1)]);
    for date in ["2024-02-29", "2000-02-29", "0001-01-01"] {
        assert_eq!(
            validate_backlog(&input.replace("2026-11-02", date), &known()),
            Ok(())
        );
    }
    for date in [
        "2026-02-29",
        "1900-02-29",
        "0000-01-01",
        "2026-13-01",
        "2026-04-31",
    ] {
        has(
            &input.replace("2026-11-02", date),
            "INVALID_FIELD",
            "/plan_date",
        );
    }
    let mut a = task("A01", 1);
    a["files"] = json!(["../outside.rs"]);
    has(&backlog(vec![a]), "INVALID_FIELD", "/tasks/0/files/0");
    assert_eq!(validate_backlog(&input, &known()), Ok(()));
}

struct TempRepo(PathBuf);
impl TempRepo {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let root = std::env::temp_dir().join(format!(
                "win-cache-planning-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    fs::create_dir(root.join("docs")).unwrap();
                    return Self(root);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("{e}"),
            }
        }
        panic!("temp dir exhausted")
    }
}
impl Drop for TempRepo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap()
    }
}
#[test]
fn markdown_link_parser_skips_code_and_understands_references_and_escaping() {
    let r = TempRepo::new();
    fs::write(r.0.join("target (a).md"), "target").unwrap();
    fs::write(r.0.join("docs/readme.md"), "").unwrap();
    // A reference definition cannot interrupt a CommonMark paragraph.
    let source = "[inline](../target%20%28a%29.md#heading)\n![image][ref]\n\n[ref]: <../target (a).md> \"title\"\n\n`[fake](missing.md)`\n```md\n[also fake](missing.md)\n```\n[external](https://example.invalid) [anchor](#x)";
    assert_eq!(
        validate_markdown_links(&r.0, &r.0.join("docs/readme.md"), source),
        Ok(2)
    );
    assert_eq!(
        validate_markdown_links(&r.0, &r.0.join("docs/readme.md"), "[bad](missing.md)"),
        Err(vec![v("INVALID_LINK", "/links/0")])
    );
}
#[test]
fn local_absolute_encoded_and_parent_escapes_are_rejected() {
    let r = TempRepo::new();
    fs::write(r.0.join("docs/readme.md"), "").unwrap();
    for target in [
        "../../outside.md",
        "/etc/passwd",
        "C:/Windows/file.md",
        "%2Fetc%2Fpasswd",
        "%43%3A%2FWindows/file.md",
        "%00.md",
        "bad%ZZ.md",
    ] {
        assert_eq!(
            validate_markdown_links(
                &r.0,
                &r.0.join("docs/readme.md"),
                &format!("[bad]({target})")
            ),
            Err(vec![v("INVALID_LINK", "/links/0")]),
            "{target}"
        );
    }
}

#[test]
fn backlog_duplicate_keys_missing_fields_and_types_are_not_erased() {
    let input = backlog(vec![task("A01", 1)]);
    for text in [
        input.replace(
            "\"owner_decision\":false",
            "\"owner_decision\":false,\"owner_decision\":true",
        ),
        input.replace(
            "\"owner_decision\":false",
            "\"owner_decision\":false,\"owner_\\u0064ecision\":true",
        ),
    ] {
        assert_eq!(
            validate_backlog(&text, &known()),
            Err(vec![v("INVALID_JSON", "/tasks/0/owner_decision")])
        );
    }
    assert_eq!(
        validate_backlog(&input.replace("\"owner_decision\":false,", ""), &known()),
        Err(vec![v("INVALID_JSON", "/tasks/0/owner_decision")])
    );
    for value in ["null", "0", "[]", "\"false\""] {
        assert_eq!(
            validate_backlog(
                &input.replace(
                    "\"owner_decision\":false",
                    &format!("\"owner_decision\":{value}")
                ),
                &known()
            ),
            Err(vec![v("INVALID_JSON", "/tasks/0/owner_decision")])
        );
    }
    assert_eq!(
        validate_backlog(
            &input.replace("\"id\":\"A01\"", "\"unexpected\":1,\"id\":\"A01\""),
            &known()
        ),
        Err(vec![v("UNKNOWN_FIELD", "/tasks/0/unexpected")])
    );
}

#[test]
fn exact_byte_caps_and_collection_limits_are_enforced() {
    assert_eq!(
        validate_label_manifest(&format!("{LABELS}{}", " ".repeat(1_048_576 - LABELS.len()))),
        Ok(())
    );
    assert_eq!(
        validate_backlog(
            &format!("{BACKLOG}{}", " ".repeat(4_194_304 - BACKLOG.len())),
            &known()
        ),
        Ok(())
    );
    let labels =
        json!({"schema_version":1,"labels":vec![json!({"name":"area:x","description":"x"});257]})
            .to_string();
    assert_eq!(
        validate_label_manifest(&labels),
        Err(vec![v("INVALID_JSON", "")])
    );
    assert_eq!(
        validate_backlog(&backlog(vec![task("A01", 1); 1025]), &known()),
        Err(vec![v("INVALID_JSON", "")])
    );
}

#[test]
fn repository_io_link_errors_and_planned_files_remain_distinct() {
    let r = TempRepo::new();
    fs::create_dir(r.0.join(".github")).unwrap();
    fs::write(r.0.join(".github/labels.json"), LABELS).unwrap();
    fs::write(r.0.join("docs/backlog.json"), backlog(vec![task("A01", 1)])).unwrap();
    fs::write(r.0.join("README.md"), "`planned/file.rs`\n").unwrap();
    assert_eq!(check_repository(&r.0).unwrap().tasks, 1);
    fs::write(r.0.join("README.md"), "[missing](missing.md)\n").unwrap();
    let errors = check_repository(&r.0).unwrap_err();
    assert_eq!(errors[0].file, "README.md");
    assert_eq!(errors[0].code, "INVALID_LINK");
    fs::remove_file(r.0.join(".github/labels.json")).unwrap();
    assert!(
        check_repository(&r.0)
            .unwrap_err()
            .iter()
            .any(|e| e.file == ".github/labels.json" && e.code == "INPUT_IO")
    );
}

#[test]
fn checker_command_is_cwd_independent_and_rejects_extra_arguments() {
    use std::process::Command;
    let r = TempRepo::new();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("check-planning")
        .current_dir(&r.0)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("planning: ok (labels=28, tasks=56, documents=")
    );
    for args in [vec![], vec!["unknown"], vec!["check-planning", "extra"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"error: USAGE\n");
    }
}

#[cfg(unix)]
#[test]
fn escaped_symlink_targets_and_required_json_symlinks_are_rejected() {
    let r = TempRepo::new();
    let outside = TempRepo::new();
    fs::write(r.0.join("docs/readme.md"), "").unwrap();
    fs::write(outside.0.join("target.md"), "target").unwrap();
    std::os::unix::fs::symlink(outside.0.join("target.md"), r.0.join("escape.md")).unwrap();
    assert_eq!(
        validate_markdown_links(&r.0, &r.0.join("docs/readme.md"), "[escape](../escape.md)"),
        Err(vec![v("INVALID_LINK", "/links/0")])
    );
    fs::create_dir(r.0.join(".github")).unwrap();
    fs::write(outside.0.join("labels.json"), LABELS).unwrap();
    std::os::unix::fs::symlink(
        outside.0.join("labels.json"),
        r.0.join(".github/labels.json"),
    )
    .unwrap();
    assert!(
        check_repository(&r.0)
            .unwrap_err()
            .iter()
            .any(|e| e.file == ".github/labels.json" && e.code == "INPUT_IO")
    );
}
