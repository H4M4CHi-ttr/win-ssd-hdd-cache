use crate::{
    ManifestViolation,
    json::{self, Node},
    violation,
};
use serde_json::{Value, json};

/// Validate only the deliberately restricted JSON-subset host workflow.
/// This is not an arbitrary YAML interpreter or a code-execution sandbox.
pub fn validate_host_workflow(input: &str) -> Result<(), Vec<ManifestViolation>> {
    let actual = json::parse(input, 65_536).map_err(|e| vec![e])?;
    compare(&actual, &recipe(), "").map_err(|e| vec![e])
}

// This reviewed recipe is intentionally independent of the actual workflow file.
// Changes to either require changing/validating the other and reviewing the diff.
fn recipe() -> Value {
    json!({
        "name": "Portable host checks",
        "on": {"push": {"branches": ["main"]}, "pull_request": {}},
        "permissions": {"contents": "read"},
        "jobs": {"host": {
            "name": "Host (${{ matrix.os }})",
            "runs-on": "${{ matrix.os }}",
            "timeout-minutes": 20,
            "strategy": {"fail-fast": false, "matrix": {"os": ["ubuntu-24.04", "windows-2025"]}},
            "defaults": {"run": {"shell": "bash"}},
            "steps": [
                {"name": "Checkout", "uses": "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1", "with": {"persist-credentials": false}},
                {"name": "Install pinned Rust", "run": "rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy --no-self-update"},
                {"name": "Record toolchain", "run": "rustc --version --verbose\ncargo --version"},
                {"name": "Format", "run": "cargo fmt --all -- --check"},
                {"name": "Lint", "run": "cargo clippy --all-targets --locked -- -D warnings"},
                {"name": "Require nonzero tests", "run": "set -euo pipefail\ncargo test --locked -- --list > \"$RUNNER_TEMP/host-test-list.txt\"\ngrep -E ': test[[:space:]]*$' \"$RUNNER_TEMP/host-test-list.txt\""},
                {"name": "Test", "run": "set -euo pipefail\ncargo test --locked | tee \"$RUNNER_TEMP/host-test-results.txt\"\ngrep -E '^test result: ok\\. [1-9][0-9]* passed;' \"$RUNNER_TEMP/host-test-results.txt\""},
                {"name": "Planning", "run": "cargo xtask check-planning"}
            ]
        }}
    })
}

fn compare(actual: &Node, expected: &Value, path: &str) -> Result<(), ManifestViolation> {
    let mismatch = || violation("WORKFLOW_POLICY", path);
    match expected {
        Value::Object(expected) => {
            let fields: Vec<_> = expected.keys().map(String::as_str).collect();
            let actual = json::object(actual, path, &fields)?;
            for (key, value) in expected {
                compare(actual[key.as_str()], value, &json::pointer(path, key))?;
            }
        }
        Value::Array(expected) => {
            let actual = json::array(actual, path)?;
            if actual.len() != expected.len() {
                return Err(mismatch());
            }
            for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                compare(actual, expected, &json::pointer(path, &i.to_string()))?;
            }
        }
        Value::String(expected) => match actual {
            Node::String(actual) if actual == expected => (),
            _ => return Err(mismatch()),
        },
        Value::Bool(expected) => match actual {
            Node::Bool(actual) if actual == expected => (),
            _ => return Err(mismatch()),
        },
        Value::Number(expected) => match actual {
            Node::Number(actual) if *actual == expected.to_string() => (),
            _ => return Err(mismatch()),
        },
        Value::Null => {
            if !matches!(actual, Node::Null) {
                return Err(mismatch());
            }
        }
    }
    Ok(())
}
