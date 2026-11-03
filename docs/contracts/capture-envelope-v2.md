# P03 — offline capture envelope and completeness audit v2

Date: 2026-11-03 JST. **Independently reviewed source contract; implementation and evidence are separate.**

This is one bounded extension of [P01 v1](capture-envelope-v1.md), motivated by [P02's explicit unknown Information](capture-producer-v1.md). The root selected this engineering approach under the owner's broad request for useful safe continuation. The owner has not specifically approved this v2 design. Contract acceptance is technical review, not owner/lab/release authorization.

## 1. Decision, alternatives and prerequisite

P02 deliberately does not inspect Information for unsupported or unsuccessful request classifications. An observed completion can therefore have a known status and unknown numeric Information. P01 v1 cannot represent that combination.

| Alternative | Decision |
|---|---|
| Replace unknown Information with zero | Rejected: invents a measured value. |
| Discard the completion and count a v1 projection loss | Honest if fully accounted, but rejected for this slice: discards observed correlation/status and creates avoidable unpaired requests plus extra projection bookkeeping. |
| Extend v1 in place to accept null | Rejected: silently changes an accepted schema's validation contract. |
| Explicit v2 with required nullable completion Information | Selected: retains the observation, separates metadata uncertainty from actual event loss, and reuses the existing bounded audit. |

Implementation starts only after P01 and P02 artifacts are accepted and this reviewed contract/planning checkpoint is published. P01 is accepted in PR81 and P02 in PR85, with actual Windows/Linux and WDK object evidence recorded separately. Independent review accepted the original v2 contract and plan before publication; see [P03 evidence](../evidence/P03.md). Minimum implementation routing is **agent:luna / review:astra** for the fixed local slice; undefined semantics escalate rather than being improvised.

## 2. Exact scope and trust

The existing API `audit_capture(&str) -> Result<CaptureAudit, CaptureError>` accepts the explicitly declared versions1 and2 after one strict typed parse. The existing `cache-capture-audit --input <ordinary-file>` needs no new option or schema-guessing path. Version1 retains its current validation and report bytes; version2 is an additive format capability.

No P02 exporter, producer/diagnostic projection, live collector, Windows API, network/device access, unsafe code, new dependency, ownership/IRQL contract, target mapping, or closure inference is added. P02 core, C/WDK helpers, scalar layouts and its producer contract remain unchanged. No D01/D04/D10/F08/F09/G1 or owner gate changes.

The checker still audits **supplied claims**, not their truth. Callers explicitly supply the existing finalized/boundary/provenance fields. There is no automatic `seal`/zero tracked contexts to finalized/zero actual inflight conversion. Candidate ranges do not become authenticated target-relative ranges. Future collection must establish those claims and preserve producer faults separately; this extension is not that collection contract.

## 3. Input contract

Inherit P01's exact root/record key sets, required field presence, value restrictions, range rules, strict objects, canonical decimal strings, evidence kinds and observation-point vocabulary, with only the versioned differences below.

- Limits remain **1,048,576 UTF-8 bytes** including whitespace and **4,096 records**.
- The schema version is an unsigned integer token1 or2. Strings, booleans, floats, exponents, negative tokens and integers beyond u64 are shape errors. Other representable unsigned versions are unsupported, not guessed from record contents.
- Parse once through the existing `Object<RawCapture>` / required `NullableString` / `UnsignedToken` adapters. Do not use generic JSON Value round-tripping, untagged fallback, or a preliminary version parse that loses duplicate-key evidence.
- Every existing field remains required even when nullable. Unknown/duplicate keys, including escaped duplicates, missing keys, positional arrays, trailing documents and wrong shapes are rejected in both versions.

| Field/phase | Version1 | Version2 |
|---|---|---|
| Submit status and information | Both explicit null | Unchanged |
| Completion status | Required nonnull canonical decimal string bounded by u32 | Unchanged |
| Completion information | Required nonnull canonical decimal string bounded by u64 | Required canonical u64 decimal string **or explicit null** |
| Completion operation, offset, length | All explicit null | Unchanged |

In v2, null Information means **this completion was observed, but its numeric Information was not reported**. It is not zero, a missing completion, failed I/O, proof of request applicability, or proof that a Windows producer correctly followed an allowlist. Numeric information remains an unmodified observation: zero, short, greater-than-requested and u64::MAX values are valid; no universal completion-status/transfer-length inference is introduced. A nonzero status does not itself make the capture incomplete. The auditor does not read any Windows status block.

Version1 null completion Information remains `INVALID_CAPTURE_VALUE`. Null completion status remains `INVALID_CAPTURE_VALUE` in both versions. Omitting either key remains `INVALID_CAPTURE`.

### Errors and precedence

Keep the existing fixed nonreflecting errors: `CAPTURE_TOO_LARGE`, `INVALID_CAPTURE`, `UNSUPPORTED_CAPTURE_VERSION`, `TOO_MANY_CAPTURE_RECORDS`, `INVALID_CAPTURE_VALUE`. Error precedence remains byte bound → typed JSON shape → supported version → record count → field values in the existing validation order. A well-shaped version3 document with4,097 records is unsupported-version, while version2 with the same records is too-many-records. Bad shape takes precedence over unsupported version; an oversized document takes precedence over either.

## 4. Correlation, counts and completeness

All existing correlation, sequence, first-occurrence ownership, counter accounting and boundary checks remain unchanged. Unknown-information completions participate normally in correlation, reserve completion identity on their first occurrence, and count as supplied completions. They do not increment lost-event counters, create gaps, fabricate submissions or leave an otherwise correctly paired request pending.

Add one diagnostic, `UNKNOWN_INFORMATION`, when **any supplied completion** has null Information. Evaluate it before duplicate-completion early return: later observations cannot erase an unknown first completion, and an unknown duplicate remains an observed uncertainty even when the first completion was known. The reason is independent of orphan/duplicate/target mismatch and actual record loss.

The exact canonical reason order is:

1. EMPTY_CAPTURE
2. NOT_FINALIZED
3. UNKNOWN_COUNTERS
4. BOUNDARY_INFLIGHT
5. DROPPED_EVENTS
6. SEQUENCE_ORDER
7. SEQUENCE_GAP
8. COUNT_MISMATCH
9. DUPLICATE_REQUEST
10. TARGET_MISMATCH
11. UNPAIRED_REQUEST
12. UNKNOWN_RANGE
13. UNCLASSIFIED_OPERATION
14. MISSING_PROVENANCE
15. UNKNOWN_INFORMATION

Version1 can never set the fifteenth reason because its null completion information is invalid. The verdict is `internally_complete` only when there are no reasons; otherwise `incomplete`. Thus a v2 capture may have complete request correlation but incomplete metadata. It does not imply that an observed flush failed or remained outstanding.

The report uses **the input's supported schema version**:1 for v1 and2 for v2. All other report keys, key order, six decimal-string counts, reason ordering, LF terminator and redaction behavior are unchanged. All four flags remain false: `producer_verified`, `authorization_verified`, `coverage_verified`, `kernel_runtime_qualified`. No input IDs, provenance strings, status or information values are echoed. No new report count, success capability or backlog mutation is introduced. `CaptureAudit` remains privately constructed with read-only existing methods.

## 5. Literal fixtures and fixed regression expectations

Create four hand-written synthetic inputs under `tests/fixtures/capture-audit-v2/`. Common root: version2; synthetic; disk_class_upper; session1; all provenance null; finalized true; start/end inflight0; attempts2/dropped0. Common identity: request1/target1, submit seq0 and complete seq1. IDs and counters use canonical decimal strings. Submit status/information and completion operation/range are explicit null.

| Input file | Submission | Completion | Expected report |
|---|---|---|---|
| `unknown-read.json` | read, offset0/length2 | status0, information null | `unknown-report.json` |
| `unknown-flush.json` | flush, offset/length null | status0, information null | `unknown-report.json` |
| `unknown-failure.json` | read, offset0/length2 | status3221225473, information null | `unknown-report.json` |
| `known.json` | read, offset0/length2 | status0, information2 | `known-report.json` |

Both literal reports have counts **records2/submits1/completions1/paired1/pending0/orphan0** and version2. The unknown report has verdict incomplete and reasons `[UNKNOWN_INFORMATION]`; the known report has verdict internally_complete and no reasons. Both include the four false flags and LF. These are original synthetic schema fixtures, not P02 runtime output or Windows evidence. Reports must be written from this contract, not generated by the implementation under test.

Additional hand-derived cases use a known-range read S and completion C for request1/target1; ordinals are explicitly0..N-1, attempts=N and dropped0 unless specified. All unrelated common root fields stay as above. Counts below are records/submits/completions/paired/pending/orphan.

| Case | Counts | Exact reasons in order |
|---|---|---|
| S, C(null), C(known); end0 | 3/1/2/1/0/0 | DUPLICATE_REQUEST, UNKNOWN_INFORMATION |
| S, C(known), C(null); end0 | 3/1/2/1/0/0 | DUPLICATE_REQUEST, UNKNOWN_INFORMATION |
| C(null), S, C(known); end1 | 3/1/2/0/1/1 | BOUNDARY_INFLIGHT, DUPLICATE_REQUEST, UNPAIRED_REQUEST, UNKNOWN_INFORMATION |
| S, C(target2/null), C(target1/known); end1 | 3/1/2/0/1/0 | BOUNDARY_INFLIGHT, DUPLICATE_REQUEST, TARGET_MISMATCH, UNPAIRED_REQUEST, UNKNOWN_INFORMATION |
| S only; attempts2/drop1/end0 | 1/1/0/0/1/0 | DROPPED_EVENTS, SEQUENCE_GAP, UNPAIRED_REQUEST |
| S, C(null); attempts3/drop1/end0 | 2/1/1/1/0/0 | DROPPED_EVENTS, SEQUENCE_GAP, UNKNOWN_INFORMATION |
| S(unknown range), C(null); not finalized; both boundaries null; windows_observation with null provenance | 2/1/1/1/0/0 | NOT_FINALIZED, UNKNOWN_COUNTERS, UNKNOWN_RANGE, MISSING_PROVENANCE, UNKNOWN_INFORMATION |
| S(other/no range), C(null) | 2/1/1/1/0/0 | UNCLASSIFIED_OPERATION, UNKNOWN_INFORMATION |

Test strict tokens/shapes/required nulls, duplicate keys, all three known-information extremes0/2/u64::MAX, null status rejection, actual loss versus unknown metadata, unchanged sequence rules, exact/+1 input limits and CLI redaction. Version1 golden report bytes and v1 value-error semantics must remain unchanged.

## 6. API, files, compatibility and acceptance

Public API is unchanged: `audit_capture(&str) -> Result<CaptureAudit, CaptureError>`, `CaptureError::code()`, `CaptureAudit::is_internally_complete()`, `CaptureAudit::to_json()`. The ordinary-file reader and CLI exit classes are unchanged:0 complete JSON;4 incomplete JSON;2 fixed usage/schema/value/UTF-8/limit error;3 fixed ordinary-file I/O error. Exit0 remains neither D01 PASS nor authorization.

Production change is local to `crates/cache-types/src/capture.rs`: carry a closed supported version through record validation/reporting, retain an unknown-information bit on completions, and append the fixed reason. No manifest/toolchain/lock/dependency or CLI production change is expected. Core/CLI test files and the six new fixtures prove the extension.

Three existing unsupported-version assertions deliberately use2: two in `crates/cache-types/tests/capture_audit.rs`, one in `crates/cache-cli/tests/capture_audit_cli.rs`. Move those witnesses to3 after contract acceptance and record the reason. This intended additive capability is the only compatibility exception; do not describe all previously unsupported inputs as behaviorally identical. Do not rewrite v1 golden bytes or change other expectations to fit implementation.

Acceptance requires contract review before implementation, observed test-first RED, unchanged fixed expectations, nonzero targeted/full tests, fmt/clippy/planning checks, independent Astra code/expectation review and actual Windows/Linux host CI logs. Public visibility and ordinary Actions permission are not new license, signing, driver/lab or release approval. The [implementation plan](../superpowers/plans/2026-11-03-p03-capture-v2.md) sequences this single unit.
