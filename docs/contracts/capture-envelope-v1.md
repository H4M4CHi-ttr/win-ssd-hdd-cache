# P01 — offline capture envelope and completeness audit v1

Date: 2026-11-03 JST. This is a **post-collection, payload-free projection**, not a kernel binary ABI, transport, producer implementation, installation recipe, F06 trace conversion or D01 acceptance. It prepares the correlation/loss evidence required by [D01](../adr/0004-interception-boundary.md) and the [probe plan](../../experiments/stack-trace/README.md). Original capture and target inventory stay local/private; this projection does not replace them.

## Scope and trust

The audit answers only: **is this supplied finite record internally complete under these declared boundaries?** It does not verify that the producer told the truth, was installed at the claimed position, observed every mutation, or was authorized. Even `internally_complete` is not PASS for any D01 matrix row. No automatic readiness, owner-approval or qualification change may consume this verdict.

The intended future collector supplies one session at one explicitly named candidate observation point and projects each observed request submit/completion into one record. Upper/lower candidates use separate envelopes; choosing a candidate here is not selecting the product position. IDs are fresh opaque session-local counters, never raw pointers, pointer hashes, device serials or path-derived values. The checker cannot authenticate their origin. Sequence is the **producer's attempted record ordinal**, not disk-apply, caller-completion, durability or cross-CPU causal order. The collector must serialize that ordinal; it must not fabricate order after capture.

Kernel forwarding, callback/IRQL/context/rundown, registration/attachment, event transport and collector quiescence are **not specified by P01**. A probe-specific reviewed contract is still required before kernel implementation. Live capture needs separately identified Windows11 targets, actual stack/build facts, installation/load/workload/trust permissions and recovery arrangements. D04/D10/G1 and owner gates are unchanged.

## Strict input

`audit_capture(&str) -> Result<CaptureAudit, CaptureError>` in `cache_types::capture` accepts at most **1,048,576 UTF-8 bytes**, including whitespace, and at most **4,096 records**. Check bytes before deserialization; the finite input bounds temporary parsing allocation, and reject the record count before building correlation state. No network, Windows API, device or unsafe code. Existing pinned serde dependencies only.

Root and records are objects, not positional arrays. Every listed key is required, including explicitly nullable keys. Reject unknown/duplicate keys (including escaped duplicate names), missing keys, wrong shapes, unknown enum strings, trailing documents, invalid JSON and version other than integer token1. No generic JSON Value round-trip may erase duplicate keys. All counters/IDs/offsets/lengths/status/information use **canonical decimal strings**, not JSON numbers: `0` or a nonzero digit followed by digits, bounded by u64. IDs are positive; completion status is additionally bounded by u32. A known range must have nonoverflowing offset+length; zero length is valid.

| Root key | Value |
|---|---|
| `schema_version` | integer token1 only (not bool, float, exponent, negative or string) |
| `evidence_kind` | `synthetic` or `windows_observation` |
| `observation_point` | `disk_class_upper` or `disk_class_lower_srb` |
| `session_id` | positive decimal-string opaque label |
| `windows_version` | explicit null or four dot-separated canonical u32 components, e.g. `10.0.26100.1`; shape validation is not Windows11 verification |
| `source_revision` | explicit null or40 lowercase hex characters |
| `producer_sha256` | explicit null or64 lowercase hex characters |
| `capture_finalized` | bool; producer claims its session record collection ended |
| `start_inflight`, `end_inflight` | explicit null or decimal strings; producer claims outstanding observed requests at the collection boundaries |
| `events_attempted`, `events_dropped` | explicit null or decimal strings, counting attempted records and lost records, including trailing loss |
| `records` | array of0..4096 objects below |

Every record has exactly these nine keys:

| Key | Submit record | Complete record |
|---|---|---|
| `seq` | decimal-string attempted ordinal, zero-based | same |
| `phase` | `submit` | `complete` |
| `request_id`, `target_id` | positive decimal-string opaque labels | corresponding request and target labels |
| `operation` | `read`, `write`, `flush` or `other` | explicit null |
| `offset`, `length` | for read/write: both decimal strings or both null; for flush/other: both null | both null |
| `status`, `information` | both null | decimal strings: unmodified observed status (u32 bit pattern) and information (u64) |

`other` retains the correlation but admits that this projection cannot classify a control/PnP/power/SRB/etc. operation. Never call it harmless. The raw capture contains the additional private classification evidence. Unknown read/write range is both-null, not zero. Known offset/length are **byte quantities in the logical address space of the named target at this observation point**; this is not proof of physical-disk mapping. If the collector cannot establish the units, target-relative address space or checked conversion to bytes from its private capture/inventory, it must emit both fields as null. P01 neither performs nor authenticates that conversion, and does not translate it to F06 offsets. Error status, short information, information greater than requested length and zero-length operations are retained as observations, not rewritten into success or rejected by an invented universal Windows completion rule. Status/information are not printed by the audit report.

Invalid format/value errors are fixed `CAPTURE_TOO_LARGE`, `INVALID_CAPTURE`, `UNSUPPORTED_CAPTURE_VERSION`, `TOO_MANY_CAPTURE_RECORDS` or `INVALID_CAPTURE_VALUE`. The checker does not echo source text, unknown keys, IDs, paths or values in errors. Error precedence is byte bound, typed JSON shape, version, record-count, then field values in declaration order. Correlation/loss defects below are not parser errors: report them as incomplete.

## Deterministic audit

Process records in supplied order, without sorting them. Bound request/completion identity sets by record count. Request IDs are unique across **all targets** within the envelope. The first submit establishes its immutable target and request record. Later submits with the same ID add `DUPLICATE_REQUEST` but never replace that record, reopen it or change pending/paired state; an ID cannot be reused after completion. Every submit/complete record still contributes to its supplied-record count. A completion pairs only with a previously submitted, still-unpaired request with the **same target**. The first completion occurrence reserves that completion ID even if orphaned/mismatched; a second occurrence never repairs the first. No missing record is synthesized.

Report decimal-string counts: records, submits, completions, paired_requests, pending_requests and orphan_completions. `pending_requests` counts distinct submitted IDs not correctly paired; orphan means a first completion with no preceding submit (a target mismatch is not an orphan). Duplicates count as supplied records but never increase paired requests. Record operation/range uncertainty is independent of correlation completeness.

Fixed correlation examples (other counter/range reasons still apply independently): submit(id1,target1), submit(id1,target2), complete(id1,target2) gives submits2/completions1/paired0/pending1/orphan0 and duplicate+target-mismatch+unpaired. Completing target1 instead gives paired1/pending0 and duplicate only. Complete(id1,target1), submit(id1,target1), complete(id1,target1) gives submits1/completions2/paired0/pending1/orphan1: the second completion cannot repair the first.

`reasons` is a deduplicated fixed-order array in exactly this order:

1. `EMPTY_CAPTURE`: no records.
2. `NOT_FINALIZED`: capture_finalized=false.
3. `UNKNOWN_COUNTERS`: any of the four boundary/loss counters is null.
4. `BOUNDARY_INFLIGHT`: either known boundary count is nonzero.
5. `DROPPED_EVENTS`: known events_dropped>0.
6. `SEQUENCE_ORDER`: any seq is less than or equal to its predecessor.
7. `SEQUENCE_GAP`: first seq!=0, any adjacent seq is not predecessor+1, or known events_attempted is not last seq+1; arithmetic overflow is a gap, never wrap. Empty records with known attempted>0 are a gap.
8. `COUNT_MISMATCH`: when both loss counters are known, checked records.len()+dropped!=attempted (overflow also mismatch). When finalized, start=0 and dropped=0, a known end count different from the actual pending count is also mismatch.
9. `DUPLICATE_REQUEST`: repeated submit ID or repeated completion ID.
10. `TARGET_MISMATCH`: a first completion names another target than its preceding submit.
11. `UNPAIRED_REQUEST`: pending requests or orphan completions remain.
12. `UNKNOWN_RANGE`: any submitted read/write has null range.
13. `UNCLASSIFIED_OPERATION`: any submit is other.
14. `MISSING_PROVENANCE`: a windows_observation lacks windows_version/source_revision/producer_sha256. Synthetic records may omit these; adding plausible metadata never authenticates them.

Verdict is `internally_complete` **only if reasons is empty**, otherwise `incomplete`. Nonzero records plus all present zero-loss/zero-boundary counters, finalized capture, contiguous attempt sequence and exact correlation are therefore necessary. This rule will conservatively reject an out-of-order asynchronous export even if its underlying I/O was valid; fix the collector contract, not the expectation.

`CaptureAudit::to_json()` returns a fixed-key-order JSON object plus LF: schema_version1, evidence_kind, observation_point, verdict, the six counts above, reasons, then **producer_verified=false, authorization_verified=false, coverage_verified=false, kernel_runtime_qualified=false**. No source IDs, metadata strings, request status or payload are echoed. Counts/flags are privately constructed by the checker; callers cannot manufacture a successful audit by mutating public fields. Provide read-only accessors needed for tests/CLI. The checker never grants a capability or edits a backlog.

## Offline CLI and fixed evidence

`cache-capture-audit --input <ordinary-file>` is a separate binary in cache-cli, reusing its existing bounded unprivileged input reader. Set the package's `default-run = "cache-cli"` to preserve existing `cargo run -p cache-cli -- ...` examples after adding a second binary; do not alter existing commands. No stdin default, directory discovery or live collection. Reject repeated/unknown/missing options and nonregular inputs; use the existing Windows namespace rejection. This is not a privileged hostile-path/TOCTOU defense and must not run elevated.

Exit0 with JSON: internally_complete (including synthetic); exit4 with JSON: incomplete. Exit2 with fixed stderr/no stdout: usage, syntax/value, UTF-8 or input limit; exit3: ordinary-file I/O failure. A zero exit is explicitly **not D01 PASS**, source authenticity or authorization.

Hand-authored fixtures must include a complete synthetic read+write pair, a windows_observation with plausible but unauthenticated metadata, a missing completion, dropped sequence, an unknown range/control and a nonfinalized capture. Tests cover the table's exact reasons/order/counts, error/short completion retention, duplicate and reordered correlation, cross-target completion, empty/unknown/boundary state, escaped duplicates at root/record, object-vs-array, required nulls, numeric/version extremes and exact/+1 byte/record limits. Mutate fixture input, not expected values derived from implementation. CLI tests inspect all exit classes, redaction, bounded files and duplicate options.

Source acceptance requires TDD, independent Astra expectation/code review and actual nonzero Windows/Linux host CI. It does **not** close D01, select an attachment point, qualify a producer, freeze a kernel ABI, authorize lab access or replace F08/F09/D04/D10/G1.
