# P04 — upper-candidate observation driver and collector v1

Date: 2026-11-03 JST. **Technical contract and bounded P04 source/build independently accepted; not runtime qualification or owner approval.** PR90 published the planning/readiness checkpoint before source work; PR91 subsequently merged the reviewed source after actual Windows/Linux host tests, enabled Win32 link and genuine WDK link. The original proposed technical text was independently accepted at SHA-256 `81467c549ff698b4ee90b2937256b1fd66dccb6e970f03dfa3df52b7b01aab60`. Later reviewed footer/PnP corrections and exact source/tree evidence are recorded in [P04 evidence](../evidence/P04.md); the original hash is not a claim that the present document is byte-identical. Accepted P01/P02/P03 artifacts remain prerequisites. Real target eligibility, attachment, installation and observation remain unqualified and separately gated.

## 1. Purpose, alternatives and authority

Build one coherent experimental source/build unit: a C17/WDK upper-candidate observation driver, bounded scalar transport, Rust collector, private native archive, and conservative P03 v2 projection. The next useful handoff is a fully source-built but **never installed or loaded** candidate, not another disconnected helper. The artifact does not select the product interception position, implement SSD caching, or complete D01/D04/D10/F08/F09/G1.

The root engineering choice is a narrow C kernel recorder and lifecycle boundary with Rust collection/audit. The owner authorized Rust-first development with justified Windows-core exceptions, but did not personally approve this exact architecture. This proposed ADR-007 exception is limited to the observation experiment. Product policy/model/service/GUI remain Rust/Tauri as already specified.

Alternatives considered:

| Alternative | Decision and reason |
|---|---|
| Put the current Rust P02 Producer behind live C/Rust pointers | Deferred. Private native-layout Ticket, large by-value initialization, stable nonpaged storage, ownership and IRQL need a new bridge. Source optimization is not a kernel-stack guarantee. |
| C-only finite observer plus Rust collector | Accepted for this bounded source/build experiment. Reuses P02 scalar extraction without live Rust pointers; C recorder receives independent literal tests. Rust Producer remains a reference, not the running producer. |
| C scalars delivered to a user-mode P02 Producer | Rejected here. Reassigning IDs/attempt counters after transport loss could conceal loss and duplicates kernel tracking. |
| ETW-only transport | Deferred. EtwWrite can succeed without an enabled session; logger loss and multi-CPU order require additional contracts. Finite synchronous polling is smaller for this candidate. |

No INF, registry-writing utility, installation, load, signature-mode change, certificate purchase, Verifier, raw-disk workload, cache hit/fill, SSD write, persistent cache reuse, public binary release, or live Actions execution. All existing owner and product gates remain unchanged. Runtime use requires separate target/build/inventory/stack/installation/load/workload/recovery authorization.

## 2. Supported source scope and explicit exclusions

- AMD64 Windows 11 client candidate; hosted Windows Server compilation is not client validation.
- One explicitly configured device instance and one capture session per driver load. No class-wide registration, first-disk selection, path-based dynamic attach, multi-target support, reset, resume, or rebind after successful removal.
- Ordinary read/write/flush/control and other non-PnP/non-power IRPs may supply metadata. No request payload, MDL mapping, DMA address, arbitrary memory, serial number, path, or pointer enters the transport.
- All own kernel code, callback contexts, recorder state and locks remain nonpaged. Own `DO_POWER_PAGABLE` stays clear and `DO_POWER_INRUSH` is never set. This is a reviewed modern-Windows experiment policy, not an assertion that flags always must match the lower stack.
- **Special-use policy A:** every `IRP_MN_DEVICE_USAGE_NOTIFICATION` with nonzero `InPath` (including BOOLEAN value2) is rejected with `STATUS_UNSUCCESSFUL`, including known types 0..7 and future values. `InPath=0` is forwarded with actual lower status/Information preserved, without reading/exporting that opaque Information, invented usage counts or power-flag transitions. This is an explicit lifecycle veto, not blanket I/O transparency. Deployment on an already boot/pagefile/hibernation/dump/special-use path is excluded and cannot be ruled out by source tests. The failure status was corrected during independent source review: Microsoft's DispatchPnP rules reserve STATUS_NOT_SUPPORTED for an unhandled request, not a deliberate handled veto. The test/source correction is tracked separately from the original fixed expectations in P04 evidence.
- Pinned WDK usage values: Undefined0, Paging1, Hibernation2, DumpFile3, Boot4, PostDisplay5, GuestAssigned6, InlineCryptoEngine7. Do not alias 4..7 or future values to Paging. PostDisplay before START is also rejected when InPath is true.
- No own hardware resources, originated storage I/O or queue is introduced. QUERY_STOP forwarding with retained observer resources is a specific resource-free-filter design inference; general function-driver stop/queue guidance is not declared irrelevant to all filters.

## 3. Exact selector and AddDevice transaction

DriverEntry copies a bounded, exact `REG_SZ` named `TargetInstanceId` under the driver's service `Parameters` key. Capacity is 512 UTF-16 code units including the sole final NUL; input must be even-sized, nonempty, terminated, and contain no earlier NUL. There is no pattern matching, whitespace trimming, expansion, default, disk-number fallback, or environment-derived value. The selector is private. Missing/malformed/query-failed configuration disables attachment; it is not successful observation.

AddDevice obtains `DEVPKEY_Device_InstanceId` from its supplied PDO with `IoGetDevicePropertyData`, with the same 1024-byte cap, required string type and valid termination. Compare exact counted strings with Windows case-insensitive comparison. Unknown/mismatch/oversized values return `STATUS_SUCCESS` without creating or attaching a filter. This follows Microsoft's filter AddDevice guideline and does not prevent the rest of the device stack from loading.

A global binding lock protects `Unbound -> Preparing -> Attached -> Retired`. Only one exact matching AddDevice can claim Preparing. A concurrent/nonmatching call skips successfully. Pre-attach preparation failure restores Unbound after complete cleanup; after a successful attach/removal, Retired never rebinds within the same driver load.

For the selected instance, the order is fixed:

1. Create an unnamed filter DO (FDO in this document), initialize its role, lower pointer, remove lock, recorder lock, PnP state and resident fixed storage.
2. Create the separate secured metadata control DO (CDO), initialized with no published target and no owner; prepare its symbolic link. Early CREATE/control is not-ready.
3. Make the final fallible call `IoAttachDeviceToDeviceStackSafe(FilterDO, PDO, &extension->Lower)`. Pass the actual field, not a local variable later copied into it.
4. On failure, delete link/CDO/FDO exactly once in reverse preparation order. No detach or remove rundown is needed because attach did not succeed.
5. On success, perform only infallible flag/characteristic initialization, publication and initialization-flag clearing. No allocation, CDO creation or other fallible call follows attach.

Copy lower buffered/direct-I/O characteristics as appropriate, retain own permanently-clear power-pageability policy, and copy applicable lower Characteristics including secure-open behavior. A dispatch injected immediately after Safe assigns Lower but before AddDevice returns must safely bypass observation using fully initialized lifetime state. It must not depend on BEGIN or late setup.

An allocation/preparation failure makes this optional observer absent rather than sabotaging the target stack. It must not be reported as attached or recorded evidence. DriverEntry/partial-failure cleanup and DriverUnload are distinct; Windows does not call DriverUnload to clean a failed DriverEntry.

## 4. Request, callback and device lifetime

### 4.1 Ordinary dispatch and completion

Every admitted forwarded IRP acquires an FDO remove-lock hold tagged by that IRP. Acquire failure during genuine teardown completes the request with the appropriate deletion status; that lifecycle rule is not an observer-allocation failure. Read/copy the current own stack while valid. The original payload, parameters, status, Information and cancel ownership are not modified by ordinary observation.

Under the short recorder lock, an eligible Running session attempts submit, reserves one of 64 metadata contexts and saves immutable classification. An observer callback context contains only the FDO pointer plus the private C slot/request-generation token. It is never transported. Callback-owned values must be published before forwarding.

Always use CopyCurrentToNext and install an all-outcomes completion routine before forwarding. If no observer slot exists or observation is stopped, install an allocation-free **release-only completion** with FDO context. Thus unobserved pending IRPs also keep the remove hold until their real completion; do not release their hold merely because IoCallDriver returned pending.

The observed callback first copies FDO/token to locals, then under recorder lock uses the same request's saved snapshot to call the existing P02 completion helper. It retires the context exactly once even if the scalar snapshot is invalid. After retirement it must never read that slot again: another IRP may reuse it before this callback releases its remove hold.

Both passthrough completions propagate PendingReturned with IoMarkIrpPending when needed, release their remove hold, and return `STATUS_CONTINUE_COMPLETION`. They never call IoCompleteRequest on the forwarded IRP, never return STATUS_PENDING, never seize cancel ownership and never retain the IRP after return. The dispatch tail after IoCallDriver may return its saved dispatch result only: inline completion may already have freed the context and released the last extension hold. No IRP/extension/context access there.

No recorder/publication lock is held across IoCallDriver, IoCompleteRequest, wait, allocation, file I/O or user output. Fixed scalar snapshots are not a validation of arbitrary pointers. Caller ownership, nonaliasing, live/stable own stack, resident memory and `<= DISPATCH_LEVEL` remain explicit C/WDK obligations.

### 4.2 PnP and power

PnP states are numeric: NotStarted0, Started1, StopPending2, Stopped3, RemovePending4, SurpriseRemoved5, Deleted6. Saved previous stop/remove states are independent of recorder state. State changes are protected by the recorder/state lock; no PnP wait holds a spinlock.

- START: hold remove lock; forward with a dedicated synchronous completion that signals a local event and returns MORE_PROCESSING_REQUIRED. Wait only if pending, at PASSIVE_LEVEL and without spinlocks. Read actual lower IoStatus.Status, set Started only on lower success, preserve lower results, complete once, release hold, and return saved result. The synchronous completion does **not** call IoMarkIrpPending; it is not the generic passthrough callback.
- QUERY_REMOVE: save prior state (including NotStarted), enter RemovePending before forwarding, set the required PnP success status, and use release-only completion. New ordinary FDO CREATE now fails; other I/O remains forwarded. Do not restore state from IoCallDriver's dispatch return; subsequent CANCEL resolves the query.
- QUERY_STOP: save prior state, enter StopPending and forward the query. The resource-free observer creates no own queue or generated I/O; the lower function driver owns hardware admission. Its resident completion/recorder resources remain alive across STOP.
- CANCEL_REMOVE/CANCEL_STOP: forward lower-first through the dedicated synchronous path. While pending, state remains pending. After lower completion restore only the matching saved state; spurious cancellation succeeds without changing state. Honor mandatory cancel-success semantics, do not invent a new capture session or resume observation.
- STOP: mark Stopped and permanently stop current observation admission; retain contexts/ring/remove holds for genuine late completion. Forward required PnP success. No free, reset, force-retire or remove-wait. A later START may restore device state but cannot reset the once-begun recorder.
- SURPRISE_REMOVAL: mark SurpriseRemoved, stop observation, forward required PnP success, but do not detach/delete. Existing callbacks remain valid until REMOVE.
- DEVICE_USAGE_NOTIFICATION: apply section 2's explicit veto policy. Record skipped/veto diagnostics, but do not invent paging counts or power-flag transitions. An observed true-veto before BEGIN prevents later BEGIN for that binding; false notification is not evidence that the disk had no prior special use.
- REMOVE: acquire its own remove hold; unpublish target first, stop recorder admission, release all spinlocks; set required PnP success and forward with SkipCurrent and **no observer completion**. Save only the dispatch return. Then IoReleaseRemoveLockAndWait at PASSIVE consumes that hold and waits for all callback/control holds. Delete link/CDO, detach Lower and delete FDO exactly once. No IRP access after forwarding, no extension access after deletion. Do not fabricate a REMOVE completion record.
- POWER on Windows 11: forwarding with the allocation-free release-only completion, no synchronous wait, no power-policy ownership, no hardware work. All touched own state is resident; test PASSIVE/APC/DISPATCH schedules. Do not turn successful host scheduling into runtime power qualification.

PnP/power requests are deliberately outside the generic observation correlation. During a Running observation, each such skipped request increments the separate skipped counter once and sets UNSUPPORTED_OBSERVATION; a true usage veto additionally sets USAGE_VETO. A stopping/sealed window is not enlarged by later unrelated requests. REMOVE before START, after failed START, directly, and after SURPRISE all need separate tests.

### 4.3 CDO ownership and removal race

CDO uses `IoCreateDeviceSecure`, an original class GUID, default SDDL `D:P(A;;GA;;;SY)(A;;GA;;;BA)`, and FILE_DEVICE_SECURE_OPEN. Reject nonempty file names. This is a default ACL, not proof of the effective runtime ACL: class-GUID registry overrides are possible and require later validation. The CDO has no data read/write operations or storage-control forwarding.

CREATE admits one FileObject at a time after publication; a second distinct FileObject gets STATUS_SHARING_VIOLATION. BEGIN additionally binds the session owner to that FileObject. Every subsequent command requires that same owner. Duplicate handles of the same FileObject do not grant a second session; command IDs are still checked. Invalid BEGIN does not consume the once-only session latch. No second BEGIN is allowed after a valid BEGIN, even after cleanup.

Control lock order is strict: acquire publication lock, validate FileObject/owner/session/call, acquire target remove hold, then **keep publication locked while acquiring the recorder lock at DPC level and performing bounded mutation/encoding**. Release recorder at DPC level, release publication/restoring prior IRQL, release target hold, then complete CDO IRP. No waits, allocation, forwarding or completion under either spinlock. This closes the delayed-BEGIN race in which CLEANUP could otherwise clear an Unarmed owner before BEGIN later starts an orphan session. Callbacks take recorder only. REMOVE clears publication under its lock, releases that lock, then stops recorder; no recorder->publication inversion and no handle-long target hold.

CLEANUP uses the same publication->remove-hold->nested-recorder order to clear only the matching owner/open-file identity and stop a live begun capture. `consumer_lost` is a no-op in Unarmed (there is no session to lose) and in already-Sealed/empty; other begun states latch CONSUMER_LOST and stop, with wrapper seal only when tracked0. Thus opening/closing before BEGIN does not poison a later valid session. True usage-veto before BEGIN is a separate sticky admission refusal. If CLEANUP wins before BEGIN, that BEGIN fails owner validation without latch/counter mutation; if BEGIN wins, CLEANUP stops that already-begun session. CLOSE is an authorization no-op: delayed A CLOSE after A CLEANUP and B CREATE/BEGIN cannot clear/stop B. An A IOCTL after cleanup is ACCESS_DENIED even if B now owns the CDO. CLOSE and post-removal CLEANUP access only CDO state, never a remembered FDO pointer. CDO deletion may be pending until existing file handles close. Unload/partial cleanup must respect those references. No terminal snapshot must survive target removal; its absence makes the collection incomplete, and removal never waits for a collector ACK.

## 5. Finite C recorder semantics

Use fixed 64 request contexts and 256 FIFO records. Drop newest on full ring; never evict a retained record or block an original I/O. `sizeof(P04Recorder) <= 40960` and total per-target extension/context allocation `<= 65536` are compile assertions. No per-I/O allocation. Initialize in already allocated resident C storage, not a large by-value temporary. All calls are externally serialized by the recorder lock.

Observer states: Unarmed0, Running1, Stopping2, Sealed3. BEGIN is the only Unarmed->Running transition and requires Started, no prior true usage veto, and fresh nonzero session ID; target_id is the opaque session-local constant1, never a device number. STOP is idempotent: Running->Stopping and Unarmed unchanged. The wrapper invokes seal when Stopping and tracked contexts reach0, including after a late completion. Sealed describes this metadata ledger only; it proves neither actual Windows drain nor coverage. Ring records may still be drained after seal. A skipped true usage-veto in Unarmed sets USAGE_VETO without incrementing skipped; begin then refuses. Running skips count once and set UNSUPPORTED_OBSERVATION, plus USAGE_VETO when applicable; Stopping/Sealed do not expand the observation window.

Private C token is `(slot:u32, request_id:u64)` scoped to this single recorder; it is not Rust Ticket ABI. Request IDs start1, never reuse, and MAX may be issued once. Saved submission includes the immutable P02 validated scalar snapshot. Submit/complete/invalid-snapshot/NoContext/drop-newest semantics match P02; invalid ticket never retires another request. A valid submit still owns a context when its submit record was dropped. A valid completion retires once even when its snapshot is invalid. Invalid submit/ticket paths latch the existing P02 uncertainty rather than assigning missing records synthetic IDs.

`attempted` is the producer's attempted observation-record count, and each attempted record's seq is its pre-increment value. `dropped` is producer loss only. Their known/unknown status is coupled exactly as P02: checked overflow clears both known values, latches COUNTER_EXHAUSTED, stops new record retention, and never wraps; existing contexts still retire and retained records drain. `popped` counts records destructively removed by transport, **not delivered/durable records**. `skipped` counts excluded lifecycle/power observations, not producer record attempts. Separate checked overflow makes popped/skipped permanently unknown with their own flags.

When all relevant counters are known, `attempted = queued + popped + dropped` with checked arithmetic. Popped and skipped have independent known bits. Do not repair counters using archive length. All faults are sticky, and known bits may only change from known to unknown. No new BEGIN/reset/counter seed is exposed through IOCTL; counter-exhaustion tests may seed private host-test state only.

Fault bit values: TRACKING_LOST0x001, PROTOCOL_FAULT0x002, COUNTER_EXHAUSTED0x004, IDENTITY_EXHAUSTED0x008, UNSUPPORTED_OBSERVATION0x010, CONSUMER_LOST0x020, SKIPPED_EXHAUSTED0x040, POPPED_EXHAUSTED0x080, USAGE_VETO0x100. Unknown fault bits are invalid wire. Known mask: COUPLED_ATTEMPT_DROP0x1, POPPED0x2, SKIPPED0x4. Unknown stored counters must be zero on the wire, not guessed numeric values. UNKNOWN Information is represented separately and is not a dropped event.

Internal C interface names are frozen in the implementation plan: WinCacheP04Token/Record/Stats/Request/Recorder and win_cache_p04_* functions. Recorder stats own the canonical session_id; driver state owns FileObject identity, LastCallId and PnP state. Scalar output encoders write only validated fixed144/96-byte regions, return void, cannot allocate/fail, and are invoked under the bounded locks after request/capacity validation. DRAIN uses one local record, pops/encodes each, then samples final stats and writes the header. No fallible encoder may lose a popped record after mutation.

## 6. Native IOCTL protocol v1 — exact byte layout

This is an experiment-local scalar wire, not D06's product ABI. All integers are unsigned little-endian. Serialize fields, not native struct bytes. Constants are frozen by this proposal before tests; changing any requires contract review. No variable strings, native pointers, padded structs or METHOD_NEITHER.

Private DeviceType0x8337; Function0x800..0x803; Method0 (METHOD_BUFFERED); RequiredAccess3 (FILE_READ_DATA|FILE_WRITE_DATA) for every command. The complete IOCTL32 is compared, not just Function. Endpoint is the one fixed CDO symbolic name `WinSsdHddCacheProbeP04`; no caller-selected path or target is accepted by the live client. Class GUID proposed for this original control device: `{d475d19a-6bb4-4d16-9b66-483e4026fa11}`; it is an identifier, not a security attestation.

| Command tag | IOCTL32 | Function | Required input | Required output capacity |
|---|---|---|---|---|
| BEGIN1 | 0x8337E000 | 0x800 | exactly48 | exactly144 |
| STATUS2 | 0x8337E004 | 0x801 | exactly48 | exactly144 |
| DRAIN3 | 0x8337E008 | 0x802 | exactly48 | exactly1680 |
| STOP4 | 0x8337E00C | 0x803 | exactly48 | exactly144 |

Request is 48 bytes:

| Offset | Bytes | Field and required value |
|---|---|---|
| 0 | 8 | ASCII `WCHP04RQ` |
| 8 | 4 | version1 |
| 12 | 4 | request_bytes48 |
| 16 | 8 | session_id, nonzero |
| 24 | 8 | target_id1 |
| 32 | 8 | call_id, nonzero; BEGIN must1 |
| 40 | 4 | requested_max_records1..16 for DRAIN, otherwise0 |
| 44 | 4 | reserved0 |

After BEGIN, all IDs must match the bound owner/session/target. A new accepted command's call_id must be greater than the previous accepted call_id; replay/duplicate is rejected before mutation. A gap permits best-effort STOP after an uncertain failed call, but the archive must retain that gap/incomplete fact. No automatic retry repeats a destructive DRAIN as if it were the same response. Invalid fields/lengths/state/owner do not pop records or advance call_id.

Successful response header is exactly144 bytes; actual returned size is `144 + 96*record_count`, at most1680. Non-DRAIN record_count is0. DRAIN may return0. Status and post-drain stats are sampled in the same recorder-lock interval as popping/encoding. Snapshot the whole request to locals first because METHOD_BUFFERED shares input/output storage. Check separate input/output capacities before any output write; zero all returned bytes and unused reserved fields. `IoStatus.Information` is exactly actual returned bytes, never capacity.

| Offset | Bytes | Response field |
|---|---|---|
| 0 | 8 | ASCII `WCHP04RS` |
| 8 | 4 | version1 |
| 12 | 4 | header_bytes144 |
| 16 | 4 | total_bytes144+96*count |
| 20 | 4 | record_bytes96 |
| 24 | 4 | record_count0..16 and <=requested_max_records |
| 28 | 4 | command tag1..4, matching actual IOCTL |
| 32 | 8 | session_id |
| 40 | 8 | target_id1 |
| 48 | 8 | exact echoed call_id |
| 56 | 4 | observer state1..3; no successful pre-BEGIN response |
| 60 | 4 | PnP state0..6 |
| 64 | 4 | active_contexts0..64 |
| 68 | 4 | queued_records0..256 |
| 72 | 4 | known mask, only0x7 bits |
| 76 | 4 | fault mask, only0x1ff bits |
| 80 | 8 | attempted, or0 when unknown |
| 88 | 8 | dropped, or0 when unknown |
| 96 | 8 | popped, or0 when unknown |
| 104 | 8 | skipped, or0 when unknown |
| 112 | 4 | exact echoed requested_max_records |
| 116 | 4 | evidence_flags0, permanently unverified |
| 120 | 24 | reserved zero |

Each record is exactly96 bytes; session/target come from its frame:

| Offset | Bytes | Field |
|---|---|---|
| 0 | 8 | seq, original attempted ordinal |
| 8 | 8 | request_id, nonzero |
| 16 | 4 | kind1 submit /2 complete |
| 20 | 4 | reserved0 |
| 24 | 4 | submit major0..255; complete0 |
| 28 | 4 | submit minor0..255; complete0 |
| 32 | 4 | submit flags0..255; complete0 |
| 36 | 4 | submit control_known0/1; complete0 |
| 40 | 4 | submit range_known0/1; complete0 |
| 44 | 4 | submit control_code; unknown/complete0 |
| 48 | 8 | submit offset; unknown/complete0 |
| 56 | 8 | submit length; unknown/complete0 |
| 64 | 4 | complete unmodified status bits; submit0 |
| 68 | 4 | complete information_known0/1; submit0 |
| 72 | 8 | complete information; unknown/submit0 |
| 80 | 16 | reserved zero |

Submit validation is P02's scalar validation, including public-control vs internal/SCSI distinction and range bounds. Complete unknown Information requires storage0. Known Information requires statusbits0. When a preceding retained submit exists, ordinary-read/write classification disagreement is an INCONSISTENT_HISTORY fact, not permission to drop or rewrite a well-shaped record; a missing submit remains correlation loss/uncertainty, not permission to synthesize one. Do not read Information for prohibited classes before deciding to redact it.

CDO errors have Information0 and no output: bad IOCTL STATUS_INVALID_DEVICE_REQUEST; wrong input/value/replayed call STATUS_INVALID_PARAMETER; wrong output capacity STATUS_BUFFER_TOO_SMALL; wrong owner STATUS_ACCESS_DENIED; unpublished/not-started STATUS_DEVICE_NOT_READY; absent/removed target STATUS_DELETE_PENDING; repeated valid-session BEGIN STATUS_INVALID_DEVICE_STATE. Default CDO read/write requests are unsupported. No control operation is pended and no cancel queue/worker exists. These calls do not forward to the storage device.

## 7. Rust collector, freshness and privileged boundary

Create two crates in one issue: `cache-probe-core` (portable `forbid(unsafe_code)`, existing pinned serde/serde_json plus cache-types and cache-capture-core) and `cache-probe-collector` (CLI with a small separately reviewed `cfg(windows)` Win32 FFI capsule). No additional third-party crate is required. The CLI denies unsafe by default and allows it only in that capsule; it does not inherit/relax forbidden-unsafe rules of existing crates. Neither crate changes Rust product code or the existing cache-cli default binary.

Win32 bindings are explicit, minimal `extern system` declarations for CreateFileW, DeviceIoControl, GetFileType (Kernel32) and BCryptGenRandom (Bcrypt), with exact Windows-width types. An OwnedHandle owns the successfully opened CDO; no raw handle escapes. Synchronous handle, no OVERLAPPED, fixed endpoint, no automatic elevation, no credential discovery, no disk enumeration or install. Cross-check declarations against primary headers/docs in source review and actual Windows link.

Live collection is a default-off Cargo feature `experimental-live`. Without it, or off Windows, `collect` rejects before any platform call. With it, the explicit `--experimental-live` switch is still mandatory; it prevents accidental use but is **not authorization**. The caller separately needs the approved lab scope.

Before opening the CDO, require inherited stdout type FILE_TYPE_PIPE; reject console, disk/device, unknown or invalid stdout. Accept no output filename or arbitrary device path. Buffer the bounded native archive in memory, then emit its exact bytes to that inherited pipe. Offline audit/project accept ordinary input files only and must run unprivileged. Do not repurpose the existing ordinary-path checker as privileged hostile-path protection. A binary-safe downstream sink is the caller's responsibility; shell text conversion is not a valid archive workflow.

Generate session_id with BCryptGenRandom(NULL, eight bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG), at most four draws; reject failure or four zero values before BEGIN. No timestamp, pointer hash, process ID or deterministic fallback. Nonzero random freshness is probabilistic, not authentication or a proof of uniqueness across all runs. Target1 is only a session-local opaque label for the configured binding.

BEGIN uses call1. Subsequent calls increment with checked arithmetic and never reuse an ID. The portable response validator receives the exact Request and actual bytes returned by DeviceIoControl; it verifies command, call, session, target, requested maximum and exact size. Any failed call is uncertain, including one that may already have popped data; latch TRANSPORT_ERROR. Bad response latches BAD_RESPONSE without archiving unvalidated raw bytes. Stop normal draining; attempt one fresh STOP best-effort if a handle exists, retain incomplete state, and never retry a failed DRAIN as delivery recovery.

CLI forms are exactly `audit --input <ordinary-file>`, `project --input <ordinary-file>`, and `collect --experimental-live --duration-ms <canonical1..30000>`. No implicit live default, stdin default, automatic open discovery, output-path option, configurable control code, session-ID override or target-ID override. Reject duplicate/unknown/missing arguments. `--help` is static and may return0 without opening anything.

Collection requests DRAIN until the duration ends; empty drains may wait10ms in user mode, never in driver. STOP then drain the already-tracked window for at most5000ms of user-mode polling. Reaching native limits or that post-STOP deadline latches incomplete and does not force-retire callbacks. A clock limit is not a guarantee that a faulty kernel call can be interrupted. Keep two command-frame slots reserved for STOP/status when approaching the frame cap; request only the remaining record budget, at most16. Budget exhaustion ends normal collection, attempts STOP, and never drops/truncates a returned record to pretend it fitted.

## 8. Native archive v1 and conservative v2 projection

Archive byte limit1,048,576; frame limit4096; record limit4096. The maximum fully populated legal framing is `64 + 4096*144 + 4096*96 + 64 = 983168` bytes. Limits include empty frames, whitespace is inapplicable, and checked arithmetic is mandatory. Archive is:64-byte prefix, zero or more exact response frames, optional64-byte footer. No arbitrary metadata strings or payload are accepted.

Prefix:

| Offset | Bytes | Value |
|---|---|---|
| 0 | 8 | ASCII `WCHP04AR` |
| 8 | 4 | archive_version1 |
| 12 | 4 | header_bytes64 |
| 16 | 4 | wire_version1 |
| 20 | 4 | response_header_bytes144 |
| 24 | 4 | record_bytes96 |
| 28 | 4 | reserved0 |
| 32 | 8 | session_id, nonzero |
| 40 | 8 | target_id1 |
| 48 | 4 | record_limit4096 |
| 52 | 4 | frame_limit4096 |
| 56 | 4 | evidence_flags0 |
| 60 | 4 | reserved0 |

Footer:

| Offset | Bytes | Value |
|---|---|---|
| 0 | 8 | ASCII `WCHP04EN` |
| 8 | 4 | version1 |
| 12 | 4 | footer_bytes64 |
| 16 | 4 | frame_count |
| 20 | 4 | record_count |
| 24 | 4 | collector_faults: TRANSPORT_ERROR1, BAD_RESPONSE2, LIMIT_REACHED4, TERMINAL_TIMEOUT8, INCONSISTENT_HISTORY16; only0x1f |
| 28 | 4 | terminal_frame_index, or0xffffffff if absent |
| 32 | 8 | same session_id |
| 40 | 8 | same target_id1 |
| 48 | 4 | terminal_received0/1 |
| 52 | 4 | output_committed0, always; never a sink/durability claim |
| 56 | 8 | reserved0 |

A terminal frame is the last received frame, after an acknowledged STOP, with Sealed/active0/queued0. Its footer must have terminal_received1 and terminal_frame_index equal to that last frame's actual zero-based index, and footer frame_count/record_count must exactly match the actually parsed archive counts. A lying footer count therefore derives INCONSISTENT_HISTORY and COUNT_MISMATCH and makes terminal_received false (MISSING_TERMINAL and UNKNOWN_COUNTERS); projected attempted/dropped remain null. In contrast, a truthful footer count with conflicting terminal producer attempted/popped accounting may still establish receipt of that terminal snapshot: retain known producer counters while deriving the independent history/count faults and denying reconciliation. This distinction was independently clarified during source work; the draft predicate was revised before its first test execution, as recorded in the evidence log. terminal_received describes receipt of those observer stats, not a qualification verdict; faults or loss may still exist. A prefix-only archive, or a missing footer exactly at a complete frame boundary, is a valid unfinished archive. EMPTY_CAPTURE means zero records, not zero frames. A truncated header/frame/footer, invalid static layout/value or trailing bytes is a fixed parse error. Never salvage a partial frame as complete. The emitter writes prefix/frames before footer and reports any stdout write/flush failure with fixed stderr/exit3; a partial footer is rejected, and output_committed remains0 even if all bytes happened to reach the pipe before an error. Do not fabricate a success footer after a write failure.

Static parsing checks byte sizes/magic/version, known enum/bit values, reserved/unused zero fields, primitive bounds, positive IDs, and immutable archive/frame session and target IDs. Those failures are INVALID_PROBE_ARCHIVE. In contrast, a second BEGIN, first frame other than BEGIN/call1, illegal command/state history, repeated/decreasing/gapped call IDs, counter regression/resurrection, lost sticky bits, footer counts/index/terminal-claim inconsistency, counter identity or popped-delta violation, out-of-order/duplicated record ordinals or request identities, and saved-classification disagreement preserve all well-shaped frames and set derived INCONSISTENT_HISTORY. A terminal record-count mismatch also derives that fault. A footer that omits that flag cannot erase it. Both audit and project use the OR of footer flags and independently derived flags. Known producer drops, gaps explained by those drops, unpaired/orphan records alone and unknown Information do not by themselves invent a collector fault; their independent reasons still apply. Never repair history by sorting, renumbering or reclassifying loss.

The first valid history frame is BEGIN/call1, Running/Started, active0/queued0, known mask7, fault mask0 and all counters0. BEGIN appears exactly once. Subsequent commands are STATUS/DRAIN/STOP only. The observer state may stay Running, move Running->Stopping/Sealed, stay Stopping, move Stopping->Sealed, or stay Sealed. Running->Stopping/Sealed on STATUS/DRAIN is allowed because a STOP/SURPRISE lifecycle request may have stopped admission independently; this does not manufacture an acknowledged collector STOP. No later command restarts or resets. STOP returns Stopping or Sealed; a received terminal still needs an acknowledged STOP frame. Sealed always has active0. PnP state values are bounded but their sampled progression is not required to show every intermediate state.

Call IDs in a coherent archive are exactly1,2,...; a greater-than gap is allowed by the kernel protocol for uncertain-call cleanup, but is incomplete archive history. For the first BEGIN baseline popped=0; each next response's known popped must equal the previous known popped plus that frame's record_count (non-DRAIN count0). Known attempted/drop/popped/skipped never decrease, known bits never resurrect, and fault bits never clear. Any change known->unknown requires the corresponding exhaustion fault. At every all-known frame, checked attempted=queued+popped+dropped. All supplied records are kept in frame/record order. First submit fixes an ID; first completion reserves its ID even if orphaned; duplicates never reopen or repair a pair, as in P01/P03. All frame target IDs are1, so a cross-target record is a static identity violation rather than a new correlation target.

`project` retains every valid scalar record in received order, including duplicates, orphans, nonzero status and unknown Information. It emits schema_version2, evidence_kind windows_observation, observation_point disk_class_upper **as an unverified candidate claim**, same opaque IDs, and null windows_version/source_revision/producer_sha256. Do not infer actual OS/source/image provenance from the collector build or runner. Submit operation mapping is read iff major3/minor0, write iff major4/minor0, flush iff major9/minor0, otherwise other (including nonzero read/write minors). Read/write offset/length are null in this initial projection: raw range candidates remain in the native archive until separate target-address-space evidence exists. Flush/other use null ranges. Complete Information is decimal string only when actually present, otherwise null.

start_inflight and end_inflight are always null in P04's initial projection. BEGIN is not proof of no older I/O; Sealed/active0 is not proof of actual Windows quiescence, especially after tracking loss. attempted/dropped come only from a structurally valid received terminal producer snapshot and remain null when terminal is absent or that pair unknown. Never derive them from archive count or fold transport loss into dropped. capture_finalized is exactly the retained_records_reconciled predicate below, not the footer's terminal_received claim alone.

All four existing verification flags remain false. A projected capture cannot become internally_complete merely because transport/correlation balanced. `retained_records_reconciled` is true **iff**: at least one record; structurally consistent terminal/footer with acknowledged STOP; no footer or derived collector fault; no producer fault in any frame; no unknown counter in any frame; terminal drop0/skip0/popped=record_count/attempted=record_count; every seq is its zero-based archive record index; exact coherent history above; and every distinct submitted request pairs once with no orphan or duplicate. Otherwise false. Range/Information uncertainty does not by itself change this declared-record reconciliation predicate; it still affects P03. It never means authentication, full I/O coverage or durability.

Native report reason conditions are exact: EMPTY_CAPTURE iff records0; MISSING_TERMINAL iff no structurally consistent terminal/footer after acknowledged STOP; COLLECTOR_FAULT iff footer-or-derived fault mask nonzero; PRODUCER_FAULT iff any response fault mask nonzero; UNKNOWN_COUNTERS iff terminal absent or any frame known mask!=7; DROPPED_EVENTS iff any known dropped>0; SKIPPED_OBSERVATIONS iff any known skipped>0; CALL_SEQUENCE_GAP iff first call!=1 or adjacent call!=predecessor+1 (overflow included); SEQUENCE_ORDER iff any seq<=its predecessor; SEQUENCE_GAP iff first seq!=0, adjacent seq!=predecessor+1, or known terminal attempted!=lastseq+1 (empty with attempted>0 also gap); COUNT_MISMATCH iff footer counts differ, any available frame counter identity/popped delta fails, or known terminal record_count+dropped!=attempted or popped!=record_count; DUPLICATE_REQUEST iff repeated submit ID or repeated completion ID; UNPAIRED_REQUEST iff pending distinct submit or first orphan completion remains. UNVERIFIED_BOUNDARIES and UNVERIFIED_PROVENANCE always apply. Every arithmetic operation is checked. Missing values do not become zero for comparisons.

`audit` returns fixed-key-order JSON plus LF. A literal example with one BEGIN, one two-record DRAIN and one terminal STOP (3 frames/2 records, coherent read pair, session7) produces exactly:

```json
{"schema_version":1,"format":"p04-native-audit","verdict":"unqualified","frame_count":"3","record_count":"2","terminal_received":true,"retained_records_reconciled":true,"reasons":["UNVERIFIED_BOUNDARIES","UNVERIFIED_PROVENANCE"],"producer_verified":false,"authorization_verified":false,"coverage_verified":false,"kernel_runtime_qualified":false}
```

Reason ordering is exactly the condition listing above, ending with the two unverified reasons. Report terminal_received is the derived structural terminal check, never the untrusted footer bit alone. No IDs, selectors, path or request status are echoed by this report.

`project` must call the accepted P03 audit on its generated envelope before output; preserve its limits1MiB/4096 and exact behavior. If the JSON projection would exceed its byte limit, report PROBE_PROJECTION_TOO_LARGE without truncating; the bounded native archive remains the original evidence. `audit` valid archive and `project` valid incomplete JSON exit4. Invalid syntax/value/limits/usage/disabled-live/RNG/stdout-kind exit2 with fixed stderr/no stdout. Ordinary-file input I/O and output I/O exit3. Live successful capture emission exits4 because this experiment remains unqualified; no live exit0. Help alone may exit0.

Fixed error strings: PROBE_USAGE, PROBE_LIVE_DISABLED, PROBE_STDOUT_PIPE_REQUIRED, PROBE_RNG_UNAVAILABLE, PROBE_INPUT_IO, PROBE_OUTPUT_IO, PROBE_TOO_LARGE, INVALID_PROBE_ARCHIVE, PROBE_PROJECTION_TOO_LARGE, PROBE_DEVICE_UNAVAILABLE. No errors echo path, raw IOCTL data, selector, instance ID, bytes or OS error text. If live BEGIN never succeeds, emit no archive; PROBE_DEVICE_UNAVAILABLE/exit3. If later collection fails, emit only the valid incomplete archive prefix/frames/footer and exit4, unless emission itself fails.

Error precedence: CLI shape/usage first. Live then checks compile/platform feature, explicit switch, pipe stdout, bounded RNG, open/BEGIN, then subsequent calls, then output; later checks must not run after an earlier failure. Offline reader first handles ordinary-file I/O/metadata/read bounds; in-memory parsing checks whole byte limit before prefix, then validates complete static frame structures/values left-to-right and count limits, then footer/trailing bytes, and only then computes history reasons. An over-byte-limit corrupt input is PROBE_TOO_LARGE; a truncated extra frame is INVALID_PROBE_ARCHIVE, not an invented valid frame-count overflow. Projection serialization/audit-size checks happen only after native validation. Semantic history faults are reports/incomplete, not parser-error precedence competitors. Output errors override a would-be exit4 with PROBE_OUTPUT_IO/exit3 and no success message.

## 9. Literal acceptance schedules

These are source/host expectations, not tests already run. Hand-written wire/archives are the oracle; neither the Rust producer nor the C encoder generates all expected answers.

| Test name | Literal expected observation |
|---|---|
| wire_lengths_and_ioctl_values | request48/header144/record96/drain1680; four IOCTL values exactly section6 |
| wire_known_read_pair | request1/session7/target1; read request1 at offset4096 length512; seq0 submit, seq1 complete status0/info512; unused/reserved bytes0 |
| producer_65th_submit | 64 valid submits then another: attempted65/drop1/active64/queued64/popped0, TRACKING_LOST; no65th token |
| producer_ring_drop_pair | 128 complete pairs then one more pair: attempted258/drop2/active0/queued256; retained seq0..255; latter valid context still retires |
| producer_stop_late_completion | submit1, STOP, new submit rejected, real complete1; attempted2/drop0/active0/queued2/Sealed; no forced completion |
| producer_duplicate_token | duplicate complete latches protocol+tracking; never releases another active token |
| unknown_information_is_completion | flush pair preserves complete status0/info null; v2 counts records2/submits1/completions1/paired1/pending0, includes UNKNOWN_INFORMATION |
| lost_first_drain_offline_adversarial | Offline adversarial archive, not the live retry algorithm: nine pairs=18 records; first16 response absent, later2 retained with seq16,17; attempted18/drop0/popped18, archive2; gap+mismatch/derived fault even with footer fault0; no invented drop16 |
| live_first_drain_failure_stops_normal_collection | After uncertain first DRAIN, issue only one fresh best-effort STOP, no further DRAIN; archive holds preceding valid frames and TRANSPORT_ERROR, no invented last2 records |
| terminal_absent | complete frame prefix without footer: terminal false, finalized false, projected attempted/drop null |
| known_to_unknown_and_never_back | overflow clears coupled count bits/values and latches exhaustion; a later known counter is inconsistent history |
| counter_identity | after first16 pop of18: attempted18/drop0/popped16/queued2; after last2: popped18/queued0 |
| rng_four_zero_draws | exactly4 RNG calls, no CreateFile/BEGIN, fixed RNG error; zero then7 succeeds on second draw only |
| default_never_opens | no feature/flag, invalid arguments or non-pipe stdout call no RNG/CreateFile/DeviceIoControl |
| archive_boundary_limits | Exactly1MiB padded input passes the byte cap but fails static framing as INVALID_PROBE_ARCHIVE;1MiB+1 is PROBE_TOO_LARGE even if corrupt. No padding is legal. Independent4096-frame/4096-record well-shaped cases may be history-inconsistent; +1 is rejected, never truncated |
| output_failure_has_no_success_claim | failure before/full/partial footer: no output_committed1, no exit0, fixed OUTPUT_IO; partial archive cannot audit as qualified |

Actual-source host WDM harness compiles the same candidate `.c` plus `upper_snapshot.c` against a deliberately isolated fake ntddk/DDI provider, invokes the registered callbacks/completions, and has no HOST-specific lifecycle branches. Fake LONG/ULONG/WCHAR widths are32/32/16, not Linux long/wchar_t. Unsupported FDO payload pointers point into inaccessible guard pages and lower-facing parameters/IoStatus use unchanged sentinels; CDO SystemBuffer alone has real bounded memory. Its evidence says simulated DDI scheduling, never real IRPs/IRQL. Actual WDK build's include manifest rejects every fake-header path and records identical source hashes.

Mandatory harness names: add_absent_no_attach, add_malformed_no_attach, add_mismatch_no_attach, add_exact_once, add_concurrent_exact_single_claim, add_create_fdo_failure, add_create_cdo_failure, add_link_failure, add_attach_failure, add_inline_dispatch_before_return; io_inline_success, io_inline_complete_pending_return, io_delayed_completion, io_cancel_and_error; observer_no_context_release_only, observer_stopped_release_only, context_retire_reuse_before_old_remove_release; remove_waits_for_tracked_completion, remove_waits_for_unobserved_completion, remove_waits_for_control_call_not_open_handle, cdo_close_after_fdo_deleted; start_lower_pending_bottom_up, start_lower_failure, cancel_remove_lower_pending_bottom_up, cancel_stop_lower_pending_bottom_up, spurious_cancels, query_stop_resource_free_forward, stop_preserves_late_callback_state, surprise_then_remove, remove_without_start; power_dispatch_irql_matrix; usage_0_through_7_true_false, usage_future_true_false, usage_noncanonical_boolean_true, usage_postdisplay_before_start; control_buffer_alias_and_truncation, control_second_fileobject_denied, control_cleanup_during_remove, cleanup_before_begin_no_session, begin_before_cleanup_stops_session, close_before_begin_does_not_poison_session, control_old_close_preserves_new_owner.

Inline callback must retire before IoCallDriver returns, with no later extension/IRP access. Pausing a callback after metadata active reaches0 but before remove-hold release must still block REMOVE. An accepted control call pauses **after publication unlock but before target-hold release**; REMOVE unpublishes and waits until that call releases it, while an open CDO handle alone never holds target rundown. Separately interleave CLEANUP before and after BEGIN's serialized publication/recorder transaction, plus delayed old CLOSE after a new owner. Use deterministic two-thread barriers at fake DDI boundaries, no sleeps; watchdog expiry is failure. Fake remove locks detect double release/incorrect tags and fake DO deletion respects open-reference delete-pending.

## 10. Completion boundary and primary references

Acceptance requires contract review, TDD red/green evidence, independent Astra code/expectation review, nonzero Linux/Windows host tests, actual pinned WDK compile/link, image/import/include-path checks, and exact source/toolchain/test evidence. No skipped/zero-test platform run is PASS. P04 done would mean source/build candidate accepted, not driver safety, target eligibility, attachment position, G1, or permission to run it.

Primary references supporting the reviewed obligations (retrieved 2026-11-03):

- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/guidelines-for-writing-adddevice-routines
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nc-wdm-driver_add_device
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iogetdevicepropertydata
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddk/nf-ntddk-ioattachdevicetodevicestacksafe
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/using-remove-locks
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-ioreleaseremovelockandwait
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iomarkirppending
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/postponing-pnp-irp-processing-until-lower-drivers-finish
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/irp-mn-device-usage-notification
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/dispatchpnp-routines
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/buffer-descriptions-for-i-o-control-codes
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/defining-i-o-control-codes
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/security-issues-for-i-o-control-codes
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdmsec/nf-wdmsec-wdmlibiocreatedevicesecure
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/controlling-device-namespace-access
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-etwwrite
- https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/unload-routine-environment
- https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-deviceiocontrol
- https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom
- https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfiletype

The exact configuration value, effective ACL, target special-use state, actual stack position, real completion/IRQL/power/remove behavior and installation/recovery procedure are intentionally unverified. Do not fill them with runner facts, a successful model, a plausible string, or a user-interface confirmation flag.
