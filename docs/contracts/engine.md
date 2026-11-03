# Engine feasibility and successor boundary — D03

Status: independently reviewed portable-host direction; Windows CI/integration pending. See [ADR-005](../adr/0005-cache-engine.md) and [evidence](../evidence/D03.md). This is not a C ABI, Windows driver contract or a replacement for D02/D07.

## 1. The executable experiment

`experiments/ocf-portability/rust_spike.rs` is original Rust, included only by the `cache-model` integration test. `#![forbid(unsafe_code)]`; no disk, file, network, thread, clock, Windows API or OCF calls. It uses ordinary host `std` allocation and can abort on actual allocator exhaustion; bounded logical resources do **not** make this a fallible kernel allocator implementation.

| Limit | Meaning |
|---|---|
| Origin | 8,192 cells, initially known zero |
| Cache | Two 4,096-cell slots, initially unknown and unbound |
| Records | 64 over the entire experiment; no ID reuse or record retirement |
| Owned cells | 65,536 logical cells, including reserved maximum caller-read snapshots |
| Counters | epoch/generation/incarnation start at the documented values; positive test limit up to u64::MAX; never wrap |

A cell is `Option<u8>`, not a physical sector. A payload buffer, transfer coverage vector, result copy and fake-medium array are separately allocated and measured. The test limits are not proposed production defaults.

## 2. Minimal submit / complete / publish API

These are Rust experiment operations, not stable product APIs. Accepted record indices are unique within the bounded run; rejected operations do not create a record. D02's wire-level rejected-ID tombstones and parent/child namespaces are intentionally absent.

| Operation | Semantic effect |
|---|---|
| `write(offset, bytes)` / `discard(offset, length)` | Check range/budget; reserve a record; invalidate every intersecting line and increment its writer count before any lower effect. Discard's effect is unknown, not zero |
| `read(offset, length)` | Admit an independent origin-read record and reserve its sample/result cells; it remains possible while the cache is disabled |
| `fill(line)` | Reject disabled/retired/active-writer lines; capture source epoch/generation before any lower sample |
| `bind(slot, line)` | Require no physical writer/reader lease; increment incarnation and invalidate even for the same binding; exhaustion retires |
| `cache_write(source, slot)` | Require a complete, successful full-known fill and a free bound slot; acquire exclusive lease, increment incarnation, invalidate old visibility and record this child as physical content owner |
| `hit(offset, length, slot)` | Only one cache line; validate active metadata, take a shared read lease and capture token. No actual bytes are supplied by the test caller |
| `transfer(id, offset, length)` | Apply/sample one disjoint fragment from the real fake-medium/source buffer; record actual coverage. Effects are distinct from a reported completion count |
| `complete(id, status, reported)` | Actual lower completion, once only; release that record's physical lease/writer count. Origin reported prefix must have actual transfer coverage |
| `publish(id)` | Successful/full-known/full physical write plus current source epoch/generation, zero writers, matching binding/incarnation/owner, no active write lease. Otherwise reject without restoring an old valid entry |
| `commit(id)` | Actual lower completion required; preserve lower origin status/count. For a cache hit require full coverage and final token/visibility validation. Store one immutable owned result and return a shared reference |
| `cancel(id)` | Store a flag only, even after completion; never synthesize actual completion or release a lease |
| `invalidate_epoch()` | Disable cache admission, purge visibility, advance epoch if possible; retain actual I/O ownership. This small experiment has no resume |

`Denied::{Bounds,State,Capacity,Rejected,Pending}` is a private experiment vocabulary. It is not the error/decision vocabulary for F05/F06, public diagnostics or a Windows NTSTATUS mapping. Actual lower statuses are only `Ok`/`IoError` here. No cancelled lower-status or raw Windows code claim is made.

Important asymmetries are deliberate:

- A stale fill may still physically write a slot; generation rejection happens at publish. This tests that metadata invalidation and content provenance protect the already-written slot, rather than hiding the schedule by an early source check.
- An origin request with reported prefix not actually applied/sampled is rejected as an impossible lower promise. A cache completion claiming full count without full physical coverage can be recorded, but may not produce a visible entry or caller hit.
- A failed origin mutation may change bytes outside its reported prefix; those effects remain in fake storage and the entire requested overlap stays invalid.
- A hit's lower completion ends its physical lease; a mutation or rebind before caller commit still invalidates its saved token. A later mutation after valid commit cannot change the stored caller result.
- `commit` returns a reference to a retained result, not a clone allocated outside the reserved budget. Result snapshots reserve their maximum logical size before admission. This does not test system allocator failures.

## 3. Non-goals of this spike

No arbitrary-size engine, replacement policy, sector alignment, last-short-line geometry, full trace-v1 parser/replay, independent oracle, durable history, FUA/flush, parent-plus-two-children fallback lifecycle, actual cancellation, resume, record retirement/reclamation, real callbacks/concurrency, production `no_std` build, pinned buffer/MDL/DMA lifetime, IRQL, HVCI, corruption checksum or low-memory kernel recovery is implemented.

The fallback test explicitly starts a separate origin read after a rejected hit. It does not prove one Windows parent can safely race a late cache child and an origin child. Cancellation stores only a flag; the test controls when the actual fake I/O completes. No cancel latency or physical cancellation guarantee follows.

Four faults are intentionally injectable only in this experiment: capture source token too late; use a writer boolean; invalidate only the first overlapped line; ignore physical provenance/old visibility. The fixed schedules must distinguish these from the correct candidate. These author-written checks are not a substitute for independent oracle acceptance.

## 4. F09 handoff after its dependencies

F09's existing dependencies remain D03/F06/F07/F08. It must make an explicit host candidate boundary and keep three responsibilities separate:

1. Scheduler chooses the input event and invokes the candidate's operation.
2. Closed fake storage executes actual fragments and completions; it provides trusted observations to D07's independent ledger.
3. Candidate emits admission/publication/result claims, which the oracle validates without reading candidate freshness state.

Reuse the four schedules as regression seeds and retain the expected failures. Do not copy `fresh`, writer-count, slot-validity or physical-owner predicates into the oracle. Define typed immutable request IDs, checked ranges, submitted descriptors and actual completion records in F09's reviewed implementation plan; do not expose this experiment's private `Record`/`Vec` layout as an ABI.

F09 must add all missing D02 semantics, resource accounting and the D07 evidence schema, then exercise the required stale-publication/early-ACK/live-lease mutants. The four D03 mutants alone do not satisfy D07's set or Windows qualification. F09 cannot declare a failing or pending trace successful merely because it parses or because the candidate rejects every operation.

## 5. W02 and optional FFI

The selected direction does not need an OCF C bridge. A later original Rust engine may share pure algorithmic code between host and kernel only after `no_std`, allocation, synchronization and lifetime review; this `std` experiment is not that library.

If D09/D04 require C/WDK, W02 must use their accepted opaque handles, fixed-width checked values, callback context and allocator contract. No unwind or Rust-owned collection crosses C. Record owner, callback IRQL, completion-before-free, cancel/rundown and caller-result immutability for every boundary. Unknown mapping remains blocked, not inferred from host success.

OCF-specific callback work is enumerated in ADR-005 but **not frozen or implemented** as a product interface. Selecting OCF later requires a reviewed ADR/dependency change and exact source/license/content-hash evidence; it is not an optional runtime switch in this experiment.
