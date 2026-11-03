# Portable host development

This checkout contains pure Rust host models, codecs and offline tools. It does not contain a qualified Windows cache driver. Never attach personal disks to test a planned driver; raw-device, installation, Verifier, signing and hardware-fault work require the separate gates and owner-authorized lab.

## Local commands

Install a standard Rust development environment for your OS; Windows builds need the ordinary MSVC linker/build prerequisites, not a WDK or administrator-run tests. The workflow-policy regressions also execute a tiny fixed Bash/grep check (Git Bash on Windows; no WSL required). On Windows they select `%ProgramFiles%/Git/bin/bash.exe` explicitly, never a PATH lookup that could select the WSL launcher. Set `WIN_CACHE_TEST_BASH` to an absolute Git Bash executable path for a nonstandard installation. On Unix, `bash`, `grep` and `cat` must be on PATH. The repository pins Rust **1.99.1**, edition 2024, rustfmt and clippy. Use Cargo.lock; do not update dependencies merely to get a green result.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo xtask check-planning
```

With dependencies already fetched, the first three accept `--offline` as appropriate. Unit/integration tests exercise in-memory data and temporary ordinary files only. Some targets have no unit tests of their own: require a nonzero total of applicable integration/unit tests, not a misleading zero-test success. Actual platform-specific test counts can differ. Offline command examples are under [status](../examples/status/README.md) and [benchmarks](../examples/benchmarks/README.md).

The [F06 replay guide](evidence/F06.md) documents the independent-history host model:

```sh
cargo run --locked -p cache-model --example replay -- --trace tests/fixtures/replay/origin-read.json
```

It accepts only an explicit bounded ordinary file and outputs payload-free evidence. Exit0 means the trace passed observed checks; exit2 is inconclusive, exit3 is a detected model failure and exit1 is input/usage/IO failure. `--fault early_ack` on `tests/fixtures/replay/early-ack.json` is deliberately exit3, not a passing candidate. `--bin cache-replay` is the same CLI used by process tests. Reports currently carry null provenance and cannot be used as release-gate proof. No device or Windows APIs are involved.

## Offline capture projection audit

[P01 evidence](evidence/P01.md) and the [capture contract](contracts/capture-envelope-v1.md) describe a separate post-collection checker:

```sh
cargo run --locked -p cache-cli --bin cache-capture-audit -- --input tests/fixtures/capture-audit/complete-synthetic.json
```

Only an explicit ordinary file is read (1 MiB/4,096-record bounds). Exit0 means **internally complete supplied records**, not truthful capture, authorization, mutation coverage or D01 PASS; all verification flags remain false. Exit4 is incomplete evidence with JSON reasons, exit2 is usage/format/value/UTF-8/limit failure and exit3 is ordinary-file I/O failure. Errors are fixed codes, not source text. Do not run elevated. There is no kernel collector or live device source, and the existing `cargo run -p cache-cli -- status ...` default remains unchanged.

## Host CI policy

The [workflow](../.github/workflows/host-tests.yml) is intentionally written in the JSON subset of YAML. Its [closed policy](../xtask/src/workflow.rs) rejects any unreviewed key, command, action, event, runner or condition. [Mutation tests](../xtask/tests/workflow_policy.rs) validate the real file and reject duplicate JSON keys, privilege/trigger/credential expansion, missing OS/checks, masked failures, altered commands and zero-test guard removal. This is not a general-purpose YAML validator.

- Only `push` to main and ordinary `pull_request`; no `pull_request_target`, `workflow_run`, reusable or manual workflow.
- GitHub-hosted `ubuntu-24.04` and `windows-2025`, x64; no self-hosted runner, container or service. Windows Server host tests do **not** qualify the Windows 11 driver target. Image contents evolve even with named runner labels.
- Workflow token permissions are only `contents: read`. Checkout uses the automatically supplied job token with `persist-credentials: false`; no configured repository/org/environment secrets, PAT, SSH or signing material are injected. “No secrets” does not mean checkout is unauthenticated.
- Only `actions/checkout`, pinned to canonical **v7.0.1** commit [3d3c42e5aac5ba805825da76410c181273ba90b1](https://github.com/actions/checkout/commit/3d3c42e5aac5ba805825da76410c181273ba90b1). On 2026-11-02 the canonical GitHub tag and action definition were checked: Node 24, ordinary hosted-runner checkout. No third-party Rust setup action is needed.
- Exact `rustup toolchain install 1.99.1 ... --no-self-update`, bounded 20-minute jobs, no `continue-on-error`. Both OS entries run fmt, clippy, actual tests and planning validation.
- A Bash `set -euo pipefail` discovery step checks test presence, but discovery alone can include ignored tests. The actual `cargo test --locked` execution is therefore also recorded via `tee` under `pipefail` and must contain an anchored successful harness result with at least one **passed** test. Zero/all-ignored execution is refused; a failing Cargo command cannot be masked by `tee`. Grep reads complete input, and behavioral shell fixtures test zero, ignored, positive and failed cases. `.gitattributes` keeps checked-in text fixtures LF on either platform.

The policy is an audit aid, not a security sandbox. It does not prove arbitrary Rust source, build scripts, procedural macros, dependencies or runner images harmless. Changing the policy and workflow together still requires independent review. There is no signing/release/deployment pipeline; do not widen this PR job to add one. Update the verified action SHA and the pinned recipe together under a reviewed dependency change.

Check [F10 evidence](evidence/F10.md) and the linked Actions run for actual execution. A workflow file or local Linux success is not Windows evidence.
