# P02 — bounded capture producer and compile-only upper snapshot v1

2026-11-03 JST. **Independent Astra accepted the contract/plan at44e7e84 and
the diagnostic/WDK clarifications at4b474ce. Source/build was subsequently accepted
in PR85; runtime acceptance remains pending.** See [P02 evidence](../evidence/P02.md).
This is a source/build preparation artifact for D01, not the adopted product
position, a loadable observation driver, a live collector, or D04's general
Windows ownership contract. Existing D01/D04/D10/F08/F09/G1 and owner gates stay
unchanged. [P01](capture-envelope-v1.md) remains a post-collection audit, not a
producer ABI or authorization.

## 1. Decision and alternatives

Continue useful preparation before target inventory with an original, bounded
Rust producer and a C/WDK **scalar snapshot object**. The alternatives were a
generic log ring (insufficient connection to the intended observation boundary)
and an installable passthrough probe (premature attachment/rundown/permission
decisions). The selected slice binds metadata to submit/completion notifications
without implementing either Windows notification callback. It is root's bounded
engineering choice under the user's continued-development/Rust-first request,
not an owner approval to install or operate a driver.

The Rust crate is `cache-capture-core`: `no_std`, `forbid(unsafe_code)`, no
dependencies, allocator, pointers, Windows API, network, locks, blocking or
mutable global. It owns fixed arrays: **64 request contexts and 256 event slots**.
An upper-candidate C object checks WDK field/type compatibility only. It has no
DriverEntry/AddDevice, registration, attachment, forwarding, completion/cancel
call, live endpoint, INF/catalog, `.sys` link or installation path. Lower/SRB is
separate and unsupported here. A future integration must independently prove
real synchronization, residency, lifetime, callback registration and rundown.

## 2. Intended observation boundary and privacy

The future caller takes a submission snapshot from its own live, stable
`IO_STACK_LOCATION` **before forwarding or changing stack location**. Completion
uses the immutable saved submission and the live completion `IO_STATUS_BLOCK`,
not stack parameters reread after forwarding. The helpers receive pointers only
for the duration of that synchronous call; they retain/export none. They do not
own IRPs, MDLs, payloads, file/device objects or their lifetime.

Caller obligations (not implemented by P02): code/input/output/saved metadata are
resident/nonpageable, IRQL is at most DISPATCH_LEVEL, output does not alias input,
and saved classification belongs to that exact request. Null checks do not
validate arbitrary pointers. No `MmIsAddressValid`, exception-based probing or
copy of a whole IRP/union substitutes for those obligations. Logging failure must
not be used as an original request status or cancellation. No zero-timing-impact
promise is made.

| Saved request | Submission fields inspected | Completion Information inspected |
|---|---|---|
| Read 0x03 / Write 0x04, minor 0 | scalar signed ByteOffset and u32 Length | only if exact status bits are 0 (`STATUS_SUCCESS`) |
| Flush 0x09, minor 0 | no Parameters union | never |
| Public DeviceControl 0x0e, minor 0 | scalar IoControlCode only | never |
| All other major/minor pairs | no Parameters union | never |

Major/minor/stack Flags are copied as u8-valued scalars in all cases. Internal
device control/SCSI share major 0x0f; do not read the public IOCTL union there.
Read/write nonzero minors are `Other`, not ordinary transfers. A nonnegative
offset and checked length are merely a **candidate byte range at that stack
location**, not proof of physical-target mapping. Negative/special offsets stay
unknown. No geometry conversion or target identity inference occurs.

`IoStatus.Information` is request-dependent: PnP device-relations can return a
kernel pointer, and failed filesystem writes can leave it undefined. Therefore
the table's success-only ordinary-read/write rule is an intentionally narrow
observer policy. For every other case the WDK helper **does not read that field**.
The allowlist branch must precede argument evaluation: do not pass Information
to a constructor and redact it later, or copy the whole IO_STATUS_BLOCK. This is
a source-access/non-export rule, not a claim about speculative CPU accesses.
Unknown information has a false presence bit and zero canonical storage; it is
not an observed zero. Status is preserved as u32 bits even on failure. In the
allowlisted case preserve the observed information without clipping it to the
requested length. This is not authenticity or protection against a malicious
driver falsely labelling values as byte counts.

## 3. Shared scalar representation, not a live binary protocol

`SubmitSnapshot` / C `WinCacheP02Submit` has size **48**, alignment **8**:

| Field | Type | Offset | Rule |
|---|---|---:|---|
| version | u32 | 0 | 1 |
| major | u32 | 4 | 0..255 |
| minor | u32 | 8 | 0..255 |
| flags | u32 | 12 | 0..255 |
| control_known | u32 | 16 | 0 or 1 |
| range_known | u32 | 20 | 0 or 1 |
| control_code | u32 | 24 | zero unless known |
| reserved | u32 | 28 | zero |
| offset | u64 | 32 | zero unless known |
| length | u64 | 40 | zero unless known |

`control_known=1` is allowed only for major0x0e/minor0. `range_known=1` is allowed
only for ordinary read/write, offset <= i64::MAX, length <= u32::MAX and checked
**u64** offset+length. Check a signed offset for negativity before converting it;
never perform this addition in signed C arithmetic. An exclusive end above
i64::MAX is allowed: offset0x7fffffffffffffff + length0xffffffff yields
0x80000000fffffffe. This preserves a candidate, not physical-range validity.
Unknown candidates are allowed even for ordinary read/write.
Flush/Other have no range; a known public control is still classified `Other`.
The C helper sets known control for public control/minor0, and known range for
nonnegative ordinary read/write offsets. Rust strictly rejects bad version,
widths, reserved bits, presence bits, inactive storage or cross-kind fields.

`CompletionSnapshot` / C `WinCacheP02Completion` has size **24**, alignment **8**:
u32 version=1 at0; u32 status at4; u32 information_known at8; u32 reserved=0 at12;
u64 information at16. Information-known must be 0/1, false requires zero storage,
and true is legal only under the table's saved ordinary-read/write/status0 rule.
That rule is independent of range_known: ordinary read/minor0 with an unknown
range may have known completion Information. Status0x103 (pending) does not
qualify. A high-bit u64 in an allowlisted successful transfer is not clipped
merely because it resembles a pointer; the request classification is the boundary.
False remains allowed even there. Decoding needs the saved validated submission;
it returns `Completion { status: u32, information: Option<u64> }` without healing
unknowns or changing statuses. Rust uses `repr(C)` integer fields, not references,
bools/enums or raw structure-byte serialization at a foreign boundary.

The ordinary C snapshot helpers take `const IO_STACK_LOCATION *` for submission
or `const IO_STATUS_BLOCK *` plus saved `WinCacheP02Submit *` for completion, and
an exclusive output pointer. They are **not WDM callback routines**. Return u32
0 means a snapshot was produced, 1 means a null argument, 2 means invalid saved
submission. A nonnull output is all-zero on error. Null-argument errors take
precedence over saved-submission validation. These codes are not NTSTATUS and
must never be used to complete/forward a request. Host scalar constructors
and validators in the same header exercise representation without WDK access.

No runtime Rust/C call or pointer bridge is introduced. Matching layouts, literal
vectors, independent source review and a real WDK object compile are the bounded
evidence. A future pointer/FFI bridge needs its own lifetime/aliasing review.

## 4. Producer API and serialization

`Producer::new(session_id: NonZeroU64, target_id: NonZeroU64) -> Producer`
creates one upper-candidate session. IDs are fresh opaque caller-provided labels,
not device/pointer/path derivatives. A caller must not reuse the same session ID
across live producer instances or retain tickets across destruction/recreation.
This is a declared identity precondition, not global uniqueness authentication.
Producer itself is neither Clone nor Copy; copying a live state is not a reset.

The API uses exclusive `&mut self` for every mutating operation. It proves a
serial host state transition, **not multicore/IRQL synchronization**. A future
caller must serialize notifications, publish the context before forwarding, and
release any producer serialization before a lower call can complete inline. It
must retain real callback context independently until actual completion/rundown.
There is no reentrant callback or external function call inside a core method.

Interfaces:

- `submit(SubmitSnapshot) -> Result<Ticket, SubmitError>`
- `complete(Ticket, CompletionSnapshot) -> Result<(), CompleteError>`
- `stop()`; `seal() -> Result<Stats, SealError>`
- `pop() -> Option<Record>`; `stats() -> Stats`

Errors are closed enums: SubmitError::{Closed, InvalidSnapshot, NoContext,
IdentityExhausted}; CompleteError::{Closed, InvalidTicket, InvalidSnapshot};
SealError::{StillRunning, Outstanding}. CaptureState::{Running, Stopping,
Sealed}. Operation::{Read, Write, Flush, Other}. SnapshotError::InvalidSnapshot.
`Record` exposes seq/session_id/target_id/request_id and
`Observation::{Submit(Submission), Complete(Completion)}`. All IDs/seq are u64;
active_contexts/queued_records are usize. No cancellation error is an I/O result.

`Ticket` is Copy but has private session/target/slot/request fields and read-only
ID getters. Request IDs start at1, increase once per reserved context, and never
reuse, including after completion. Matching all four fields is required. Slot
reuse does not make an older ticket current. Copy permits explicit duplicate
notification tests; it is not a physical I/O lease or permission to free memory.

`Record` holds seq/session/target/request IDs plus a validated `Submission` or
`Completion` observation. It contains no pointer, payload or string. Submission
retains the scalar classification/range candidate/control code and immutable
request identity. Exporting a record does not release its request context.

States are Running → Stopping → Sealed. `stop` is idempotent and only stops new
submissions. It never fabricates completion or releases contexts. Matching
completions remain accepted while Stopping. `seal` fails with `StillRunning` in
Running and `Outstanding` if any tracked context remains. Otherwise it seals;
repeated seal is idempotent. There is no force-reset, cancel-to-complete or
force-free API. After Sealed, submit/complete return Closed without modifying the
capture; draining records via pop remains possible. A sealed producer is **not
proof that actual Windows I/O or callbacks have drained**.

## 5. Attempted order, loss and invalid notifications

Attempted event ordinal starts at0. Each accepted submission notification and
each first matching completion consumes exactly one attempted ordinal, including
events dropped for capacity or invalid completion metadata. It is the serialized
observer order, not a disk apply/durability/cross-CPU causal clock.

Running submit first validates the snapshot. Invalid input latches both
`protocol_fault` and `tracking_lost`, returns InvalidSnapshot and consumes no
ordinal/ID. A valid notification consumes an attempt. If no context is free or
request identity is exhausted, drop that event, latch `tracking_lost`, and return
NoContext/IdentityExhausted. Otherwise reserve the lowest free slot, immutable
submission and fresh ID before returning its ticket; event-ring failure does
not revoke that ticket or context. Context exhaustion takes precedence over
identity exhaustion. Outside Running, submit returns Closed before decoding.

Completion first matches the ticket; invalid/duplicate/wrong-session tickets
latch `protocol_fault` and `tracking_lost`, consume no ordinal and change no
context. A matching completion consumes one attempt and **retires that context
exactly once even if the completion snapshot is invalid**. Invalid completion
metadata drops that event, latches protocol_fault, and returns InvalidSnapshot;
a second notification cannot repair it. A valid completion preserves status and
optional information. Return values concern observation only, not original I/O.

The ring is FIFO, drop-newest, no overwrite. A full ring increments dropped once
for that attempted event. `pop` releases only its event slot. Every actual event
retained in the ring has its original ordinal; export never sorts or renumbers.
When counters are known, attempted = queued + already-popped + dropped. Core
does not claim a consumer preserved popped records; P01 must detect later loss.

Attempted/dropped counters use checked u64 arithmetic. On inability to represent
an increment, **both counters become None permanently**, new event storage stops,
and `counter_exhausted` stays true; existing records remain drainable and tracked
completions still retire contexts. Request ID u64::MAX may be assigned once;
further assignment is refused, never wrapped. `identity_exhausted` is diagnostic.
It becomes true when the final ID is assigned, not only on a later rejection.
Private unit-test state injection may reach these limits; no public seed/reset
API is provided. Flags are monotonic within a producer.

`Stats` is a copy with state, active_contexts, queued_records, attempted/dropped
Option<u64>, tracking_lost, protocol_fault, counter_exhausted and
identity_exhausted. It does not report actual start/end_inflight,
capture_finalized, producer/auth/coverage/qualification, or a PASS verdict.
In particular, context exhaustion is **not merely ring loss**: zero retained
contexts later cannot establish the completion of untracked real requests.

## 6. P01 compatibility boundary

P02 does not generate P01 JSON or call its audit. P01 cannot express an unknown
completion Information value. A future collector must preserve that uncertainty
or explicitly count an unrepresentable record as loss; it must not emit an
invented `information="0"`, bypass loss counters, or call a producer seal a
Windows quiescence certificate. Observed target mapping, collector transport,
actual boundary counters and original private evidence remain separate work.
This slice deliberately supplies no successful synthetic capture as real evidence.

## 7. Fixed regression expectations

| Case | Hand-derived result |
|---|---|
| Two contexts completed in reverse order | submit seq0/id1, submit seq1/id2, complete seq2/id2, complete seq3/id1; attempts4/dropped0/active0 |
| Reuse freed slot; old ticket completes again | old notification rejected; new context remains; no extra attempted event; protocol/tracking flags set |
| 128 pairs without pop, then two more pairs | attempts260/dropped4/queued256/active0; retained seq0..255; tracking_lost=false |
| 64 submits, rejected65th, then64 completions | attempts129/dropped1/queued128/active0; seq64 absent; tracking_lost=true permanently |
| Stop with one outstanding | new submit refused without counter change; seal refuses; late matching completion retires once; then seal succeeds |
| Invalid completion for one valid submit | attempts2/dropped1/active0; first submit only retained; duplicate cannot repair; protocol fault |
| Ordinary read succeeds with information greater than length | observed scalar preserved, not clipped; no universal transfer-validity claim |
| Other/PnP-like, flush, failure or nonzero-minor completion | information None with zero inactive storage; pointer-looking value never exported |
| Negative offset / foreign invalid fields | helper yields unknown candidate; decoder rejects bad shape/width/presence/reserved/cross-kind values |
| Counter and ID limits | no wrap/reuse; unknown counters stay unknown; outstanding contexts can still retire |

Tests must check all fields/order, not only counts. Additional cases cover ring
drain/reuse, copied tickets across distinct session IDs, invalid submission,
immutable sealed state, failed completion status bits, zero length, exact layout,
and `size_of::<Producer>() <= 40_960` on tested x64 hosts. That size is a host
budget check, not an approved kernel allocation/stack-use budget. No performance
or hardware safety claim derives from these tests.

## 8. Build evidence and unresolved deployment

Test-first original Rust and C scalar code; independent Astra expectation/source
review; actual nonzero Windows/Linux host CI; C literal-vector host executable;
and AMD64 C17 `/c /kernel /W4 /WX` WDK object compile. Use the existing D09-pinned
Microsoft package versions/digests and NuGet signature checks. No LLVM install or
kernel image link is needed for this object. Inspect COFF/object symbols for
unexpected external/lifecycle/I/O symbols, record compiler/header/source hashes,
and upload only text/JSON evidence, never binaries or real target data. A symbol
allowlist is a structural check, not runtime verification.
Keep the platform-default header packing; do not force /Zp8 or pragma pack to
obtain alignment8. Assert natural layouts instead. /kernel defines _KERNEL_MODE;
do not define it manually or mix host CRT typedefs into the WDK translation unit.

The pinned wdm.h defines select-any pool-support data merely when NTDDI_VERSION
>= NTDDI_WIN2K. An actual C17 /kernel experiment with documented `/Zc:inline`
still emitted this data. Restore the original `/O2` command rather than stack
further flags or manipulate the SDK/target definitions. The specific header-data
role below is a reviewed correction to the overly narrow initial object model,
not a general exception for unfamiliar external symbols. Its diagnostic evidence
and failed flag experiment remain in [P02 evidence](../evidence/P02.md).

The object checker accepts at most1MiB of ordinary AMD64 COFF: machine0x8664,
1..96 sections, zero optional-header size, bounded section/symbol/string tables,
valid auxiliary-symbol skipping and bounded external names. PE/bigobj variants
are outside this check. Required defined external **function** symbols are exactly
`win_cache_p02_snapshot_submit` and `win_cache_p02_snapshot_complete`; they must
be function symbols in real sections. Permitted undefined externals are only
`__security_cookie`, `__security_check_cookie`, `__chkstk`, `memcpy`, `memset`.
Reject weak externals, common externals and any other external name or role,
except for zero or one precisely shaped header-data definition:

| Element | Required shape |
|---|---|
| External data symbol | Exact name `ExPoolZeroingNativelySupported`, storage class2, type0 (not a function), value0, no auxiliary record, positive bounded section index |
| Its section | Exact short name `.bss`, size1, raw pointer0, physical/virtual address0, relocation and line pointers/counts0; characteristics exactly0xc0101080 (uninitialized, select-any COMDAT, one-byte alignment, read/write, not executable) |
| Section definition | Exactly one static class3 symbol in that section, name `.bss`, type0/value0, exactly one bounded auxiliary record |
| Section auxiliary | Length1, relocation/line counts0, checksum0, selection2 (IMAGE_COMDAT_SELECT_ANY). Association/reserved fields are not interpreted; select-any is not an associative COMDAT. |

For that optional section, the first non-auxiliary symbol referencing it must be
the unique static section definition above. The next non-auxiliary symbol
referencing it must be the permitted data external. Auxiliary records do not
count in this section-local order; unrelated symbols referencing other sections
may appear between them. This identifies the external's COMDAT role without
requiring global adjacency or hardcoding a compiler-selected section index.

The optional data name cannot substitute for either required function or appear
twice, as an undefined/weak/common/function symbol, or in another section shape.
No extra undefined runtime calls are admitted. The report retains `defined` for
the two functions and adds `header_data: []` or the single exact name, separately
from `undefined`. This is an explicit pre-acceptance checker-report amendment;
it does not change either snapshot layout or a public runtime ABI. Synthetic
fixtures must accept the observed bounded form and reject near-miss roles,
sections, duplicate/missing definitions, wrong section-local order and auxiliary
metadata. Positive cases also retain unrelated interspersed symbols and nonzero
uninterpreted association/reserved auxiliary fields. No existing
other rejection expectation is relaxed. Unrelated non-external compiler metadata
is still not interpreted as executable semantics.
The checker does not certify relocations, memory accesses, general COFF validity,
loadability, ABI calls, header provenance or runtime safety. Name/shape alone
does not authenticate origin; independent source review and pinned package/header
hashes establish the present build evidence. Further allowlist changes need
review, not an automatic response to a failed build.

Still required before any runnable probe: specified Windows11 inventory/target,
actual stack/order and registration effects, approved disposable lab/operations,
reviewed real forward/pending/callback/context/IRQL/rundown integration, collector
transport and permission, lower/SRB analysis, rollback and coverage interpretation.
P02 does not choose or authorize any of these. Source/build preparation itself
does not require inventing those deployment facts.

## Primary references checked by root, 2026-11-03

- [IRP_MJ_READ](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/irp-mj-read) / [IRP_MJ_WRITE](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/irp-mj-write): scalar request and transfer information; not physical mapping proof.
- [IO_STACK_LOCATION](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/ns-wdm-_io_stack_location) / [current stack helper](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iogetcurrentirpstacklocation): major/minor-specific union and own stack.
- [IRP codes](https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/-irp): internal/SCSI alias; inspect pinned WDK declarations too.
- [IO_STATUS_BLOCK](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/ns-wdm-_io_status_block) / [device relations](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/irp-mn-query-device-relations): Information is request-dependent and can be a pointer.
- [Archived filesystem write](https://learn.microsoft.com/en-us/previous-versions/windows/drivers/ifs/irp-mj-write): counterexamples to assuming all minor/status/offset combinations are ordinary valid transfers; not a disk-stack observation.
- [Completion routine](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nc-wdm-io_completion_routine) / [dispatch IRQL](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/dispatch-routines-and-irqls): residency and IRQL preconditions remain real integration obligations.
- [IoCallDriver](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iofcalldriver) / [IoCancelIrp](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iocancelirp): handoff/access and cancel are not completion/lifetime proof.
- [MSVC /kernel](https://learn.microsoft.com/en-us/cpp/build/reference/kernel-create-kernel-mode-binary?view=msvc-170), [C17](https://learn.microsoft.com/en-us/cpp/build/reference/std-specify-language-standard-version?view=msvc-170), [packing](https://learn.microsoft.com/en-us/cpp/build/reference/zp-struct-member-alignment?view=msvc-170): compiler-defined kernel mode, C scalar assertions and platform-default SDK alignment.
- [MSVC /Zc:inline](https://learn.microsoft.com/en-us/cpp/build/reference/zc-inline-remove-unreferenced-comdat?view=msvc-170): unreferenced COMDAT code/data removal, command-line/MSBuild default difference and optimization restrictions.
