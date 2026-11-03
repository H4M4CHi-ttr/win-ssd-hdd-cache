# P04 Upper-Candidate Probe and Collector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Issue #89 as one never-loaded, source-built observation driver plus finite Rust collector, with evidence-preserving offline audit/projection and independently reviewed tests.

**Architecture:** C17/WDK owns only this experiment's IRP/lifecycle/fixed recorder boundary; portable Rust validates the wire/archive and projects P03 v2. The actual driver source is tested through an explicitly simulated DDI harness and independently compiled/linked with pinned WDK. No test result is Windows runtime qualification.

**Tech Stack:** Rust1.99.1, existing pinned serde1.0.229/serde_json1.0.154, C17, pinned Microsoft SDK/WDK packages, hosted MSVC with recorded identity, Node built-ins for build/evidence tooling. No new third-party dependency; Windows collector uses a small reviewed extern-system capsule.

**Spec:** `docs/contracts/upper-candidate-probe-v1.md`; language exception `docs/adr/0018-capture-probe-language-boundary.md`. Intended plan path: `docs/superpowers/plans/2026-11-03-p04-upper-probe.md`.

**Status:** Independently accepted technical plan, proposed-content SHA-256 `4a1ba0e18646fd56c6d503924027d983b35eb8ca0f2d6ac02866218c59bbc4eb`. Source work waits for publication/readiness of this checkpoint. P01/P02/P03 are accepted; P03 merged at `17edb16ba14f6f4029c6485cb0cb652fc7cd4bd5` and root verified the actual CI evidence. No P04 source/test/build is claimed by this plan. See [P04 evidence](../../evidence/P04.md).

## Global Constraints

- One bounded Issue #89 / one PR containing the complete source/build candidate; internal task commits are not substitute Issues.
- Request48 / response-header144 / record96 / maximum-DRAIN1680 bytes; all fields LE; IOCTLs0x8337E000/0x8337E004/0x8337E008/0x8337E00C.
- Recorder64 contexts /256 FIFO records /16 records per DRAIN; drop-newest and loss/unknown counters exactly as the spec, no Rust private Ticket ABI.
- Archive limit1,048,576 bytes /4096 frames /4096 records; maximum legal complete framing983168 bytes. Prefix64/footer64.
- Exact nonempty configured instance only; absent/malformed/mismatch means STATUS_SUCCESS/no attach. Never first-device selection or automatic target discovery.
- Permanently nonpaged own kernel code/state, own DO_POWER_PAGABLE clear, no DO_POWER_INRUSH. All nonzero InPath additions, including2 and future usage types, explicitly vetoed; existing special-use targets excluded by deployment preconditions.
- No INF/install/load/signing/boot/Verifier/raw-disk workload/cache mutation/public binary release. Live feature default off; CI compiles it but never executes its device path.
- Portable core forbids unsafe; only the new CLI's cfg(windows) capsule may allow documented unsafe. Existing workspace crates' unsafe rules/default-run remain unchanged.
- No new crate versions or unrelated lock changes. Shared Cargo manifests/lock are root-owned integration points.
- Literal expectations are written before implementation and never changed to match code. Independent Astra review covers both expectations and source.
- P04 source/build acceptance never changes D01/D04/D10/F08/F09/G1 or owner-approval state.

## Review Focus

- Inline lower completion can retire/reuse observer storage and release the last remove hold before IoCallDriver returns: Task3 asserts no later IRP/extension/context access.
- Prepared CDO/link can receive early CREATE while attachment is not committed: Task3 asserts not-ready and complete pre-attach rollback, including concurrent exact AddDevice claims.
- Well-shaped but inconsistent frames may lie via an empty footer fault mask: Task4 derives faults independently and preserves records rather than repairing history.
- Collector output can fail after some bytes or a full footer reached the pipe: Task5 reports OUTPUT_IO without exit0/output-committed or durability claims; partial framing is never qualified.
- New device-usage enum values and noncanonical true BOOLEANs can bypass an equality-to1 check: Task3 covers0..7,0x7fffffff and InPath0/1/2 without fabricated paging counts or power flags.

---

## File and ownership map

| Files | Responsibility |
|---|---|
| `experiments/stack-trace/probe/include/p04_wire.h`, `probe/common/wire.c` | Frozen request/header/record LE encoding and request validation; no WDK ownership |
| `probe/include/p04_recorder.h`, `probe/common/recorder.c` | Fixed C metadata recorder and private scalar token; no OS calls |
| `probe/include/p04_driver.h` | Internal FDO/CDO roles, locks, context layouts and cross-TU driver prototypes |
| `probe/wdk/driver.c` | One actual lifecycle TU: entry/config/unload, transactional AddDevice/publication, ordinary/completion/power, dedicated PnP, CDO ownership/IOCTL/cleanup |
| `probe/host/wire_vectors.c`, `recorder_vectors.c` | Hand-derived scalar/wire/recorder tests |
| `probe/host/driver_schedules.c`, `ddi_fake.c`, `platform.c`, `platform.h`, `fake-wdk/ntddk.h`, `fake-wdk/wdmsec.h`, `fake-wdk/devpkey.h` | Same-source simulated-DDI lifecycle tests, deterministic barriers and guard-page sentinels; never real kernel evidence |
| `probe/tools/test-host.mjs`, `build.ps1`, `test-build-contract.mjs` | Host runner; ephemeral WDK build/link/evidence; workflow/build contract tests |
| `crates/cache-probe-core/src/lib.rs`, `wire.rs`, `archive.rs`, `project.rs` | Forbid-unsafe portable native protocol, bounded history, audit and v2 projection |
| `crates/cache-probe-core/tests/wire.rs`, `archive.rs`, `project.rs` | Literal native bytes/goldens and adversarial history expectations |
| `crates/cache-probe-collector/src/main.rs`, `capture.rs`, `input.rs`, `win32.rs` | CLI dispatch, portable capture orchestration, unprivileged ordinary-file input, narrowly allowed Windows FFI |
| `crates/cache-probe-collector/tests/cli.rs` | Process-level redaction, modes/limits/exits and feature-negative tests; never real CDO calls |
| `tests/fixtures/capture-probe-v1/read-pair.hex`, `flush-pair.hex`, `lost-drain.hex`, `unfinished.hex`, `report-read-pair.json`, `project-read-pair.json`, `project-flush-pair.json` | Literal independent native archives and fixed output bytes, not generated from production encoder |
| `.github/workflows/probe-build-proof.yml`, `docs/evidence/P04.md` | Hosted-only proof and acceptance record |
| Root `Cargo.toml`, `Cargo.lock`; both new crate manifests | Integration only; no dependency upgrades |

Existing P02 snapshot files, D09 `tools/pins.json`, `tools/pe-check.mjs` and its22tests are consumed unchanged. Do not weaken their checks or rewrite prior evidence. `probe/` paths above are under `experiments/stack-trace/`.

## Task 1: Freeze literal wire fixtures and implement both wire boundaries

**Files:** Create both new crate manifests/minimal entry files, `cache-probe-core/src/wire.rs`, its `tests/wire.rs`, all listed literal fixture files, `probe/include/p04_wire.h`, `probe/common/wire.c`, `probe/host/wire_vectors.c`, and the host test runner. Root registers the crates and lock serially. Minimal skeletons only enable test discovery; do not implement before RED.

**Interfaces:**

- Rust `Command::{Begin,Status,Drain,Stop}` has tags1..4 and `Command::ioctl()->u32`. `Request::new(command: Command, session: NonZeroU64, call: NonZeroU64, maximum: u32) -> Result<Request, ProbeError>` fixes target1. Request is Copy; `Request::to_bytes() -> [u8;48]` and `Request::ioctl() -> u32`.
- Rust `decode_response(expected: Request, actual_bytes: &[u8]) -> Result<Frame, ProbeError>` validates fixed shapes and exact binding. Frame/Record fields are exposed read-only; `Frame::records()->&[Record]` and `Record::information()->Option<u64>` support tests. No authenticated/qualified constructor exists. `ProbeError::code() -> &'static str` returns only fixed spec strings.
- C scalar types use existing WinCacheP02U8/U32/U64 aliases and are never serialized as native structs. `WinCacheP04Token={slot:u32,request_id:u64}`; `WinCacheP04Record={seq:u64,request_id:u64,kind:u32,submit:WinCacheP02Submit,complete:WinCacheP02Completion}`; `WinCacheP04Stats={session_id:u64,observer_state:u32,active_contexts:u32,queued_records:u32,known_mask:u32,fault_mask:u32,attempted:u64,dropped:u64,popped:u64,skipped:u64}`; `WinCacheP04Request={command:u32,session_id:u64,target_id:u64,call_id:u64,max_records:u32}`.
- `WinCacheP04WireResult` is a WinCacheP02U32 typedef. `WinCacheP04WireResult win_cache_p04_decode_request(WinCacheP02U32 ioctl,const WinCacheP02U8 *bytes,WinCacheP02U32 input_bytes,WinCacheP02U32 output_capacity,WinCacheP04Request *out)` validates exact IOCTL/48-input/exact144-or1680-output before mutation. Result constants WIN_CACHE_P04_WIRE_OK0, WIN_CACHE_P04_WIRE_BAD_IOCTL1, WIN_CACHE_P04_WIRE_BAD_INPUT2, WIN_CACHE_P04_WIRE_BAD_OUTPUT3 are local results, not NTSTATUS.
- `void win_cache_p04_encode_header(const WinCacheP04Request*,const WinCacheP04Stats*,WinCacheP02U32 pnp_state,WinCacheP02U32 record_count,WinCacheP02U8 output[144])`; `void win_cache_p04_encode_record(const WinCacheP04Record*,WinCacheP02U8 output[96])`. These internal encoders assume validated scalars/writable regions, zero/write only their fixed region, and cannot fail/allocate after a pop. DRAIN uses one local record, not a large stack array.
- Core lib uses existing cache-types, cache-capture-core and workspace serde pins. CLI depends on core only, with default features empty and `experimental-live=[]`; it denies unsafe by default. Core lib explicitly forbids unsafe. No Windows crate is added.

- [ ] Write literal hex fixtures from spec offsets and fixed sessions/requests; first complete archive is BEGIN1, DRAIN2 containing read submit(seq0,id1,offset4096,length512) and complete(seq1,id1,status0,info512), STOP3 with Sealed/active0/queued0/attempt2/drop0/pop2/skip0.
- [ ] Write `wire_lengths_and_ioctl_values`, `wire_known_read_pair`, `wire_binding_and_reserved_rejected`, `wire_actual_returned_bytes_not_capacity`, `wire_unknown_information_distinct_from_zero`. Assert exact constants, full byte equality and fixed errors, not a round-trip alone.

```rust
assert_eq!(Command::Begin.ioctl(), 0x8337_E000);
assert_eq!(Command::Drain.ioctl(), 0x8337_E008);
assert_eq!(request.to_bytes().len(), 48);
assert_eq!(frame.records().len(), 2);
assert_eq!(frame.records()[1].information(), Some(512));
assert_eq!(unknown.records()[1].information(), None);
```

- [ ] Run `cargo test -p cache-probe-core --test wire --locked`; record legitimate missing API RED. Run `node experiments/stack-trace/probe/tools/test-host.mjs --suite wire`; require RED due missing implementation, not absent compiler/runtime. Runner prints each named case and a nonzero count.
- [ ] Implement exactly the typed wire functions and strict masks/zero-field/domain checks. Root reviews manifest/lock and exported API consistency before later tasks consume them.
- [ ] Rerun those two commands; require every fixed case PASS, no ignored/skipped cases. Compile C with C17 warnings-as-errors, Linux and Windows at integration time.
- [ ] Commit only this task's paths after independent expectation/source review, e.g. `feat(p04): define finite probe wire and literal fixtures`.

## Task 2: Implement the real finite C recorder against fixed schedules

**Files:** `probe/include/p04_recorder.h`, `probe/common/recorder.c`, `probe/host/recorder_vectors.c`; extend host runner without changing Task1 expectations.

**Interfaces:**

- `void win_cache_p04_init(WinCacheP04Recorder*)`; `int win_cache_p04_begin(WinCacheP04Recorder*,WinCacheP02U64 session_id)`. Recorder is a complete in-place type in p04_recorder.h with root-owned private layout; driver uses operations/stats only. Begin rejects non-Unarmed/zero/prior USAGE_VETO. Target1 is implicit.
- `int win_cache_p04_submit(WinCacheP04Recorder*,const WinCacheP02Submit*,WinCacheP04Token*)`; `int win_cache_p04_saved_submit(const WinCacheP04Recorder*,WinCacheP04Token,WinCacheP02Submit*)`; `void win_cache_p04_complete(WinCacheP04Recorder*,WinCacheP04Token,const WinCacheP02Completion*)`. Saved submit is a copy, never a slot pointer. NULL completion snapshot means invalid snapshot and retires a valid token exactly once.
- `void win_cache_p04_stop(WinCacheP04Recorder*)`; `int win_cache_p04_seal(WinCacheP04Recorder*)`; `void win_cache_p04_skip(WinCacheP04Recorder*,WinCacheP02U32 usage_veto)`; `void win_cache_p04_consumer_lost(WinCacheP04Recorder*)`; `void win_cache_p04_stats(const WinCacheP04Recorder*,WinCacheP04Stats*)`; `int win_cache_p04_pop(WinCacheP04Recorder*,WinCacheP04Record*)`.
- All int returns are1 success/0 reject-or-empty; inspect fixed stats to distinguish errors. Wrapper invokes seal after stop/last completion only when tracked0. consumer_lost is Unarmed no-op and Sealed+empty no-op; other begun states latch+stop. Skip true in Unarmed latches USAGE_VETO without incrementing skipped. No IOCTL seeds/reset. All functions are serial/no allocation/no WDK/no callbacks; caller synchronizes.

- [ ] Write named schedules `producer_65th_submit`, `producer_ring_drop_pair`, `producer_stop_late_completion`, `producer_duplicate_token`, `producer_invalid_snapshot_retirement`, `producer_pop_not_context_retirement`, `producer_counter_overflow`, `producer_last_id_once`, `producer_popped_and_skipped_overflow`. Assert exact contents/flags/state in addition to counts.

```c
assert(stats.attempted == 65 && stats.dropped == 1);
assert(stats.active_contexts == 64 && stats.queued_records == 64);
assert((stats.fault_mask & WIN_CACHE_P04_TRACKING_LOST) != 0);
/* Independent 128 pairs + one dropped pair schedule: */
assert(stats.attempted == 258 && stats.dropped == 2);
assert(stats.active_contexts == 0 && stats.queued_records == 256);
```

- [ ] Run `node experiments/stack-trace/probe/tools/test-host.mjs --suite recorder` and record implementation-missing RED.
- [ ] Implement bounded context/ring/counter operations, compile-time40960/65536 budgets and invalid-token protections. Test-only private state seeding stays in host test code, never in driver control or HOST algorithm branches.
- [ ] Rerun recorder/wire suites; also run `cargo test -p cache-capture-core --locked` as unchanged reference regression. Similarity to P02 is corroboration, not the sole oracle.
- [ ] Commit reviewed paths, e.g. `feat(p04): add bounded C observation recorder`.

## Task 3: Implement the complete C/WDK candidate with source-linked lifetime tests

**Files:** `probe/include/p04_driver.h`; the single `probe/wdk/driver.c`; `probe/host/{driver_schedules,ddi_fake,platform}.c`, `platform.h`, `fake-wdk/{ntddk,wdmsec,devpkey}.h`; host runner. Reuse `experiments/stack-trace/wdk/upper_snapshot.c` unchanged.

**Interfaces:** DriverEntry(PDRIVER_OBJECT,PUNICODE_STRING), P04Unload(PDRIVER_OBJECT), P04AddDevice(PDRIVER_OBJECT,PDEVICE_OBJECT); P04Dispatch/P04DispatchPnp/P04DispatchPower/P04DispatchControl(PDEVICE_OBJECT,PIRP) -> NTSTATUS; P04ObservedCompletion/P04ReleaseCompletion/P04SyncCompletion(PDEVICE_OBJECT,PIRP,PVOID) -> NTSTATUS. `p04_driver.h` owns internal role/state/context definitions, not a public ABI. Ordinary helpers remain private to one driver.c. Control owner validation+remove-hold acquisition+bounded recorder mutation/encode keep publication locked; recorder is nested with KeAcquireSpinLockAtDpcLevel/ReleaseAtDpcLevel. Release both locks before target hold and IRP completion. Callbacks take recorder only; REMOVE unpublishes then releases publication before recorder stop.

- [ ] Write the spec's complete named schedule set into `driver_schedules.c` before driver algorithms. The fake provider registers/invokes actual source callbacks and checks held tags, access-after-release, pending flags, object refs, publication, output buffer bounds, payload sentinels and requested DDI order. Use fixed32/32/16-bit LONG/ULONG/WCHAR.
- [ ] Pin AddDevice failure literal order: CreateFdo, Initialize, CreateCdo, CreateLink, AttachFailure, DeleteLink, DeleteCdo, DeleteFdo; targetNULL; detach0; rundown0; leaked objects0. Concurrent exact matches attach exactly once; malformed/mismatch create0/attach0.
- [ ] Pin inline lifetime literal order: AcquireHold, CopyNext, InstallCompletion, CallLower, CompletionSnapshot/Retire, MarkPendingIfNeeded, ReleaseHold, ReturnFromLower, ReturnSavedStatus. The harness poisons retired context/extension/IRP access so a dispatch-tail read fails.
- [ ] Pin REMOVE/control schedules with deterministic two-thread barriers: active-context0 alone does not release callback remove hold; unobserved release-only callback also keeps its hold; accepted control is paused after publication unlock/before target-hold release and blocks deletion; open handle alone does not hold target; cleanup/close after FDO deletion access no FDO. `cleanup_before_begin_no_session` denies delayed BEGIN without mutation; `begin_before_cleanup_stops_session` begins then stops; `close_before_begin_does_not_poison_session` leaves Unarmed/fault0/once-latch clear; `control_old_close_preserves_new_owner` preserves B after A cleanup/late close and rejects old A IOCTL.
- [ ] Pin dedicated PnP schedules: lower-pending START/CANCEL does not publish Started/restore state early; synchronous completion signals+MORE_PROCESSING_REQUIRED and marks pending0; general completion with PendingReturned marks exactly1. STOP preserves storage for late completion; START after STOP cannot resume the recorder. QUERY_STOP has no own queue/resource teardown; lower status is preserved.
- [ ] Pin usage matrices: types0..7 and0x7fffffff crossed with InPath0/1/2. Nonzero => STATUS_UNSUCCESSFUL, lower_calls0, usage-veto diagnostics, no invented paging count, power flags unchanged. Zero => original lower result/opaque Information unchanged, Information not exported. PostDisplay/nonzero before START follows the veto. The status is an independently reviewed correction to the original STATUS_NOT_SUPPORTED expectation, based on Microsoft's handled-PnP failure rules; require a new failing test before updating the source.
- [ ] Run `node experiments/stack-trace/probe/tools/test-host.mjs --suite driver` and record RED due missing production driver callbacks. Do not accept failures from fake-provider width or build setup mistakes as semantic RED.
- [ ] Implement entry/config/transactional attachment, release-only and observed completions, exact dedicated PnP/power paths, default-SDDL CDO and FileObject-bound protocol. All fallible preparation precedes Safe(FilterDO,PDO,&ext->Lower); post-attach work is infallible. No after-forward object access or lock-held forwarding/wait.
- [ ] Rerun driver/wire/recorder suites. Barrier timeout is failure; no sleeps or disabled cases. Independent Astra review explicitly checks the fake boundary's blind spots and actual source against primary DDI docs.
- [ ] Commit reviewed files, e.g. `feat(p04): add never-loaded upper-candidate lifecycle and control source`.

## Task 4: Implement native archive/history audit and v2 projection

**Files:** `cache-probe-core/src/{archive,project}.rs`, `lib.rs` exports; `tests/{archive,project}.rs`; fixed fixture goldens from Task1 only if they were not yet authored. Expectations must not be generated from implementation.

**Interfaces:** `decode_archive(&[u8]) -> Result<Archive,ProbeError>`; `audit_archive(&[u8]) -> Result<NativeAudit,ProbeError>` with `NativeAudit::to_json()->String`; `project_v2(&[u8]) -> Result<String,ProbeError>`. Archive/NativeAudit fields are privately constructed. `CaptureBuilder::new(NonZeroU64)->Self`, `next_request(Command, maximum:u32)->Result<Request,ProbeError>`, `receive(Request,&[u8])->Result<(),ProbeError>`, `fail(Request,CollectorFault)->Result<(),ProbeError>`, `finish(self)->Result<Vec<u8>,ProbeError>`, and read-only remaining-record/frame/terminal/normal-drain-state accessors support Task5. Only one request is outstanding; after uncertainty only one fresh best-effort STOP is legal.

- [ ] Write `read_pair_report_literal`, `flush_pair_projects_null_information`, `prefix_only_is_unfinished`, `frame_boundary_without_footer`, `lost_first_drain_offline_adversarial`, `footer_cannot_hide_derived_faults`, `second_begin_is_history_not_parser_error`, `regression_resurrection_and_call_replay`, `counter_identity_and_popped_delta`, `duplicate_and_orphan_correlation`, `native_and_projection_limits`, `malformed_precedence`, `builder_failed_drain_allows_only_stop`. Exactly1MiB padded malformed input yields INVALID_PROBE_ARCHIVE;1MiB+1 yields PROBE_TOO_LARGE. No valid archive reaches the byte cap because legal framing maximum983168; independent4096frame/4096record boundary cases may be history-inconsistent but must remain well-shaped. No padding acceptance or false valid-cap expectation.
- [ ] Assert complete read fixture report matches the exact JSON+LF in spec, records2/frames3/terminal true/reconciled true with only the two unverified reasons. Its v2 audit reasons are exactly UNKNOWN_COUNTERS, UNKNOWN_RANGE, MISSING_PROVENANCE. Flush fixture v2 audit reasons are exactly UNKNOWN_COUNTERS, MISSING_PROVENANCE, UNKNOWN_INFORMATION; correlation remains paired1/pending0.

```rust
assert_eq!(audit_archive(&read_pair)?.to_json(), expected_literal_report);
let projected = project_v2(&flush_pair)?;
assert!(!cache_types::capture::audit_capture(&projected)?.is_internally_complete());
assert_eq!(projected, expected_literal_flush_json);
assert_eq!(audit_archive(&too_large).unwrap_err().code(), "PROBE_TOO_LARGE");
```

- [ ] Run `cargo test -p cache-probe-core --test archive --test project --locked` and record legitimate missing-function RED.
- [ ] Implement strict framing before history; preserve well-shaped inconsistent frames, derive faults independently of footer, implement the exact reconciliation predicate/reason order, and project every record in input order. Both boundaries remain null; no source/build/hash provenance is invented. Run the accepted P03 audit on generated JSON; never truncate to its limit.
- [ ] Rerun all core tests plus `cargo test -p cache-types --test capture_audit --locked`; existing v1/v2 goldens remain unchanged.
- [ ] Commit reviewed files, e.g. `feat(p04): audit native capture history without promoting runtime truth`.

## Task 5: Implement fail-safe CLI and compile-only live collector capsule

**Files:** `cache-probe-collector/src/{main,capture,input,win32}.rs`, `tests/cli.rs`, crate manifest features; no existing cache-cli behavior change.

**Interfaces:** `ProbeTransport::exchange(&mut self, request:Request)->Result<Vec<u8>,TransportError>` returns only actual bytes; `CaptureClock::{elapsed_ms(&self)->u64,wait_ms(&mut self,u32)}`; `SessionSource::draw(&mut self)->Result<u64,SessionError>`. `fresh_session(&mut impl SessionSource)->Result<NonZeroU64,CollectError>` allows at most4 draws. `collect(&mut impl ProbeTransport,&mut impl CaptureClock,session:NonZeroU64,duration_ms:u32)->Result<Vec<u8>,CollectError>` returns a bounded incomplete/terminal native archive after BEGIN; failed BEGIN returns no archive. All these orchestrators are safe and fake-testable. `win32.rs` alone owns reviewed CreateFileW/DeviceIoControl/GetFileType/BCryptGenRandom declarations and OwnedHandle borrowing.

- [ ] Add process tests `usage_never_collects`, `live_disabled_before_platform_calls`, `audit_and_project_exit4`, `redacted_input_and_limits`, `duplicate_options_rejected`, and Windows namespace rejection without opening the paths. Default build tests never call the fixed CDO.
- [ ] Add safe orchestration tests `rng_four_zero_draws`, `rng_zero_then_seven`, `failed_begin_emits_nothing`, `live_first_drain_failure_stops_normal_collection`, `deadline_keeps_late_requests_unresolved`, `budget_stops_without_truncation`, `mismatched_call_id_stops`, `output_failure_has_no_success_claim`. Fake transports capture exact command/request order. Expected failed-drain sequence is BEGIN1, DRAIN2 failure, STOP3 only; no DRAIN4.
- [ ] Run `cargo test -p cache-probe-collector --locked`; record missing-binary/API RED. Do not let feature-disabled cfg accidentally run zero relevant tests.
- [ ] Implement exact CLI forms/exit classes; ordinary-file offline input is bounded/unprivileged, not hostile elevated-path defense. Live parser rejects default/no-feature/non-Windows cases before platform work; feature-enabled live path checks pipe stdout before RNG/open. No output filename, arbitrary device path or ID override.
- [ ] Implement synchronous fixed-endpoint Win32 capsule and nonzero BCrypt session generation. Keep archive in memory; write prefix/frames before footer to inherited pipe only, check write/flush results, output_committed always0. Device unavailability before BEGIN is fixed error/no archive; later failure produces valid incomplete evidence or output error, never fabricated records or live exit0.
- [ ] Rerun CLI/core tests and `cargo check -p cache-probe-collector --features experimental-live --locked` on Windows CI. The latter is compilation only; do not invoke `collect` on the runner. Test enabled orchestration through fake transport only.
- [ ] Commit reviewed files, e.g. `feat(p04): add bounded offline and opt-in experimental collector CLI`.

## Task 6: Prove full build scope and hand off without runtime claims

**Files:** `probe/tools/build.ps1`, `test-build-contract.mjs`, host runner final integration; `.github/workflows/probe-build-proof.yml`; `docs/evidence/P04.md`; root-only backlog/handoff/README references. Reuse D09 PE checker and pins, P02 include-path discipline without changing their semantics.

**Interfaces:** `node .../test-host.mjs --suite all` compiles/runs named host suites and emits machine-checkable nonzero counts; `pwsh .../build.ps1` refuses outside ephemeral Windows Actions, verifies package hashes/signatures, builds actual kernel TUs and links never-loaded AMD64 Native image, records evidence, and never installs/executes that image. `node --test .../test-build-contract.mjs` checks workflow permissions/artifact allowlist/no-live commands/source include boundaries and source manifests. Existing `inspectPe` is a DLL-allowlist structural precheck, not an imported-function allowlist or runtime proof.

- [ ] Write build-contract tests first, including refusal to mix fake-WDK headers into actual kernel build, identical actual-source hashes between host harness and WDK source list, no invocation of live collect, no public binary upload, no broad permissions, nonzero named-test proof, and honest runtime-false flags.
- [ ] Run `node --test experiments/stack-trace/probe/tools/test-build-contract.mjs` and record missing-workflow/script RED. Existing D09 `node --test experiments/rust-wdk/tools/test-pe.mjs` stays22 cases and unchanged.
- [ ] Implement hosted Linux/Windows C and Rust host tests; actual Windows WDK compile/link with warnings-as-errors, source/header/library/compiler/image hashes and dumpbin imports. Reuse commit-pinned checkout/upload actions and exact existing package pins/signature verification. Keep contents:read and upload only text/JSON evidence.
- [ ] Run full local available verification: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo test --workspace --locked`; core/collector release tests; all host C suites; build-contract and PE tests; `cargo run -p xtask --locked -- check-planning`. Record actual case counts, ignored0 and platform limitations.
- [ ] Publish the source branch/PR for authorized Actions; inspect actual Windows C host schedules, Rust tests, enabled-live compile, pinned WDK link and PE/check evidence. Diagnose failures without weakening expectations. Await independent Astra whole-diff/expectation review after all fixes.
- [ ] Record exact immutable source/tree/workflow-run/artifact/reviewer identifiers, failed attempts and remaining unknowns. P04 evidence must say driverInstalledOrLoaded=false, realDeviceAccess=false, observedLiveRequests=false, runtimeQualified=false, windowsClientQualified=false. User-mode fake harness execution is separately identified as simulated.
- [ ] Integrate only through repository-authorized PR workflow; close #89 only after accepted artifacts/merge and honest metadata. No gate/owner approval inference. Final handoff lists the genuine next Windows inventory/target/attachment/permission/runtime work rather than suggesting the candidate was run.

## Self-review and execution boundary

The six tasks are reviewable internal increments of one coherent source/build deliverable. Scalar/fixture/parser work can be assigned to Luna after this contract is fixed; full WDM/lifetime/control integration is SOL-or-higher with independent Astra review. Model labels are minimum difficulty, not permission to skip the complete review.

Root and the independent reviewer checked spec-to-task coverage, exact cross-TU names, native report literals, history/error distinctions and the actual-source harness boundary. A newly discovered ownership/IRQL/PnP/control ambiguity stops only the affected source task for contract review; it is not patched by silently inventing hardware facts. Implementation still waits for this checkpoint's publication/readiness; no runtime operation is authorized by the plan.
