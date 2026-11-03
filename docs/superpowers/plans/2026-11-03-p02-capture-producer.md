# P02 capture producer implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Independent Astra review precedes implementation and acceptance.

**Goal:** Build a bounded metadata producer and non-loadable WDK snapshot object before live probe integration.

**Architecture:** The no_std Rust core owns fixed request/event storage and validates copied integer snapshots. A C17 helper reads only allowlisted WDK scalar fields; separate host vectors and object compilation test the limited representation. No live pointer bridge or driver callback is implemented.

**Tech Stack:** Rust1.99.1 / no_std / forbid unsafe / zero new dependencies; C17; existing D09-pinned WDK/SDK packages; ordinary hosted Actions only.

**Spec:** [capture-producer-v1](../../contracts/capture-producer-v1.md), normative after independent review.

## Global constraints

- Exactly64 request contexts /256 FIFO event slots; drop newest; no allocator/locks/OS access in Rust.
- Submission48/8 and completion24/8 scalar layouts; explicit unknown Information, no generic pointer-valued export.
- No registration/attachment/DriverEntry/live endpoint/image link/install or safety-setting changes.
- No P01 JSON projection, product position, real callback/rundown claim or existing gate changes.
- Preserve manual expectations on failure; fix defects or re-review a contract instead of fitting expected data to code.

## Review focus

- Context exhaustion must keep tracking uncertainty after retained contexts drain: core saturation test.
- Failed/unclassified completion Information can be undefined/pointer-valued: Rust/C scalar vectors and WDK branch review.
- Duplicate/stale tickets must not retire a newer request in the reused slot: exact record/context test.
- Counter/ID exhaustion must not silently wrap or prevent owned-context retirement: private limit-injection unit tests.
- Compile-only artifacts can accidentally acquire runtime entry/I/O calls: structural object/symbol checks and negative checker tests.

## Task 1 — strict scalar representation

Files: create `crates/cache-capture-core/Cargo.toml`, `src/lib.rs`, `src/snapshot.rs`, `tests/snapshot.rs`; register in root Cargo.toml/lockfile. No new registry dependency.

Interfaces: public repr(C) `SubmitSnapshot`/`CompletionSnapshot` fields exactly as spec; `decode_submit(SubmitSnapshot) -> Result<Submission, SnapshotError>` and `decode_completion(CompletionSnapshot, &Submission) -> Result<Completion, SnapshotError>`. Submission exposes read-only scalar accessors; Completion preserves u32 status and Option<u64> information. Single error is InvalidSnapshot.

- [x] Add literal layout/ordinary/unsupported/negative/unknown/cross-kind/boundary tests before implementation. Exact size48/24, align8, offsets from contract; saved PnP/Other cannot export pointer-like0xffff8123456789ab as known Information. A correctly classified successful ordinary read/write preserves any observed u64. Range validation uses checked u64 addition, not signed addition.
- [x] Run `cargo test -p cache-capture-core --test snapshot --locked`; observe missing exports as RED after package/lock setup (setup failure is not feature RED).
- [x] Implement strict scalar decoding, without unsafe/serde/Windows imports.
- [x] Re-run snapshot target and fmt/clippy; preserve first hand-derived expectations.

## Task 2 — bounded serial producer

Files: create `src/producer.rs`, `tests/producer.rs`, `src/limits_tests.rs`; export core API from lib.rs.

Interfaces: Producer::new(NonZeroU64,NonZeroU64); submit/complete/stop/seal/pop/stats, private-field Copy Ticket and the exact state/error/stat/record types in the contract. Lowest-free context reservation, nonreused request IDs, independent FIFO event lifetime.

- [x] Write public black-box schedules for every contract §7 row. Fix exact seq/request/status/unknown/order values and all count/flag assertions before coding. RED must name absent producer API.
- [x] Implement the minimum fixed-array state machine; no external callbacks and no implicit quiescence verdict.
- [x] Add private-state limit tests before adding checked-limit handling: u64::MAX request ID once, attempted overflow makes both counters None, tracked completion still retires. No public seed/reset API.
- [x] Run all core tests in debug/release; fmt and clippy. Assert object size<=40960 on x64. Review lifetime boundaries, not just happy-path counters.

## Task 3 — C scalar helpers and WDK object

Files: `experiments/stack-trace/include/p02_snapshot.h`, `host/snapshot_vectors.c`, `wdk/upper_snapshot.c`, `tools/build.ps1`, `tools/check-object.mjs`, `tools/test-object.mjs`, README; `.github/workflows/capture-build-proof.yml` and narrow .gitignore output entry.

Interfaces: C layouts/static assertions match Rust. Scalar constructors/validators initialize all fields. WDK submission helper receives const IO_STACK_LOCATION/output; completion receives const IO_STATUS_BLOCK/saved submission/output. Result0/1/2 is metadata-only, never NTSTATUS. Unknown Information is not read on the WDK branch.

- [x] Write independent literal C vectors before header implementation; compile locally with C17 W4-equivalent flags and observe missing header/functions RED. Run exact values without WDK/device access. Final20+36 cases pass optimized/UBSan host runs.
- [x] Implement shared scalar header, then C helper; do not introduce pointers into exported snapshots or read unsupported union members. Independent integration review remains below.
- [x] Write structural checker tests RED: valid synthetic COFF passes; PE image, wrong architecture, missing required helper, unknown external, DriverEntry and I/O/rundown symbols fail. State checker limitations. All14 checker tests pass; actual WDK object/dumpbin evidence remains pending.
- [x] Implement object checker and ephemeral build script using existing verified D09 pins; no LLVM/kernel-driver install. C host executable runs literal tests, `/c /kernel /W4 /WX` produces an object only; evidence contains source/compiler/header/object hashes and explicit false runtime flags. This checks source presence, not actual WDK build success.
- [x] Independent source review before publishing/running ordinary Actions. Actual WDK compile/object inspection accepted in PR85; host vectors do not execute IRPs. See final P02 evidence for the failed and successful runs.

## Task 4 — integration and acceptance

### Reviewed object-model correction after actual SDK evidence

The original object expectation omitted a pinned WDK header's select-any BOOLEAN
definition. Actual `/Zc:inline` experimentation did not remove it. The contract's
precise optional header-data role, not another compiler guess, is the bounded
correction; obtain independent contract acceptance before implementation.

- [x] Preserve the diagnostic run and unsuccessful flag experiment. Revert only
  the attempted `/Zc:inline` flag; do not change SDK files, target macros or C code.
- [x] Extend the original hand-constructed two-section fixture, not generated
  compiler bytes. Before changing the checker, observe RED for accepting the
  contract's exact one-byte BSS/header-data form. Document the intentional change
  from the previous blanket extra-data rejection and add `header_data` to the
  exact successful report witness. Other legacy expectations stay unchanged.
- [x] Reject wrong name/type/storage/value/aux, duplicate header-data, missing
  mandatory functions, near-miss section size/name/address/flags/pointers/counts,
  absent/duplicate/wrong static section definition, wrong auxiliary
  length/counts/checksum/selection, and unrelated external functions/data.
- [x] Reject the external before its section definition and any other primary
  symbol referencing that section before the definition or between definition
  and external. Accept unrelated symbols from other sections interspersed, and
  nonzero uninterpreted association/reserved auxiliary fields; do not invent
  extra constraints or require globally adjacent records.
- [x] Implement only the optional data role and selected section-aux validation;
  keep all bounds and other defined/undefined symbol policies. Do not interpret
  unrelated static metadata or claim relocation/runtime verification.
- [x] Run the complete checker suite with no ignored/skipped tests; independent
  review must examine contract, synthetic geometry and negative coverage.
- [x] Require fresh actual Windows C56/path17/checker tests, WDK compile and
  object check with separate `header_data`, exact manifest and all false runtime
  flags. A synthetic PASS alone cannot accept the actual build.

- [x] Record TDD checkpoints, literal vector counts, exact local commands, unknowns and independent review in `docs/evidence/P02.md`.
- [x] Run full workspace tests/fmt/clippy/planning, standalone C vectors and object checker; inspect nonzero named tests rather than repeated summary lines.
- [x] Independent Astra expectation/code/workflow review, repair and re-review findings.
- [x] Publish exact reviewed tree via normal PR; inspect Windows/Linux host and compile-only proof logs/artifacts. Never install/load the result.
- [x] Merge normally, update manifest/Issue/roadmap, read back roles/states/all dependency annotations. Do not turn D01/D04/D10/F08/F09/G1 ready. PR85 source and PR87 metadata were normally merged; Issue83/86/parent1 were exactly read back.

The next runtime slice still needs separately reviewed real context/callback/transport integration and the user-designated Windows inventory/lab authority. Completing this plan is not completion of a runnable probe.
