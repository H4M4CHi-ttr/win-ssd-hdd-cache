# ADR-005: Original Rust clean-cache feasibility before an OCF Windows port

Status: code/design independently accepted; Windows host CI and integration pending. 2026-11-02. Issue [D03 #8](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8).

## Decision and its boundary

Select an **original, minimal Rust clean-read-cache / Write-Invalidate policy for the next host qualification work**, subject to this ADR's review and D03's host acceptance gates. Do not select an OCF Windows port for this development increment. This is a choice of implementation direction, **not** a production engine release, Windows driver acceptance, or evidence that OCF is defective.

The executable [feasibility experiment](../../experiments/ocf-portability/README.md) is a closed 8 KiB fake-storage experiment. It demonstrates a small submit/transfer/actual-complete/publish interface and selected D02 counterexamples. It is deliberately neither a complete engine nor D07's independent oracle. Do not rename it into a production crate and declare F09 complete.

The project continues to use Rust for portable/model/service code and Tauri for a future non-privileged GUI. D09, D01 and D04 decide the actual Windows language boundary, interception location and lifetime/IRQL contract independently. This decision does not authorize a kernel implementation before G1.

## Options and evidence quality

| Option | Evidence actually obtained | Decision at this gate |
|---|---|---|
| OCF Write-Invalidate port | Pinned upstream source/API/license review. No OCF build, wrapper, run, memory measurement or performance comparison | No-Go for immediate adoption; reconsider only with the adapter and evidence below |
| Original minimal Rust policy | 14 host tests in debug/release; four fixed D02 schedules reject four deliberate faults, with a separate superseded-publish mutant check; actual fake-storage copies and completion ordering | Go for further host development, not product qualification |
| Write-back or persistent SSD metadata reuse | Outside the accepted initial-product safety contract | No-Go; not an implementation shortcut |
| User-mode folder mirroring or ordinary Windows storage tiering | Different interception/consistency semantics | Not a substitute for this engine contract |

There is no measured claim that the Rust option is faster, smaller than OCF at equal cache capacity, or more reliable in Windows. OCF's minimum cache size alone makes the tiny experiment an invalid like-for-like memory benchmark.

## Pinned OCF source audit

Reviewed upstream: `Open-CAS/ocf`, commit `94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db`. The version macros in this development snapshot say 26.12.0; this is not a claim that a stable release by that name was adopted. All links below resolve to that immutable commit, not a floating branch.

- [LICENSE.md](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/LICENSE.md): BSD-3-Clause terms; source/binary notice retention, non-endorsement and disclaimer conditions remain applicable. Source review does not choose this project's license or authorize vendoring. GOV01 remains separate.
- [WI implementation](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/src/engine/engine_wi.c) and [WA implementation](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/src/engine/engine_wa.c): WA uses the write-through path for a cache hit and WI for a miss. WA is not the requested invalidate-on-every-origin-mutation policy. A port must explicitly select and test WI rather than infer it from a name or default.
- WI's core-completion error branch and its success-path metadata purge are different. A Windows adapter must establish what happens when an origin request changes bytes and then completes with an error. This is an **unresolved compatibility question**, not a demonstrated upstream stale-read bug: the surrounding locking, request lifecycle and supported backend error semantics must be tested together.
- [I/O API](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/inc/ocf_io.h): `ocf_end_io_t` and `ocf_forward_end` carry an error, not the Windows reported transfer count. Passing that error through is insufficient to prove INV-02. A wrapper would need separate trusted lower status/count/effect records and a reviewed completion policy; it may not manufacture full success from a short request.
- [Read/backfill](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/src/engine/engine_rd.c) and [backfill completion](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/src/engine/engine_bf.c): source data is copied into a backfill buffer, foreground completion and later backfill are distinct, and successful backfill updates metadata. These are useful design precedents, not proof of the Windows parent/child rundown contract.
- [Definitions](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/inc/ocf_def.h): minimum cache volume 40 MiB and line sizes 4/8/16/32/64 KiB. [Private sector definitions](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/src/ocf_def_priv.h) use 512-byte sectors, while an `OCF_BLOCK_SIZE_4K` option separately selects internal block size. [I/O creation](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/src/ocf_io.c) checks 512-byte address/length alignment. None of this establishes compatibility with every 512e/4Kn pair or Windows transfer alignment.
- [Cache management defaults](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/inc/ocf_mngt.h): default cache mode resolves to WT and `metadata_volatile` defaults to false. WI and volatile metadata must be selected explicitly rather than assuming the default matches this product. Backfill limits also require project-specific bounds; upstream defaults are not a Windows nonpaged-pool budget.

No upstream source was copied into the repository and no OCF dependency was added. Any later package dependency requires an exact version/content digest and license review; vendoring additionally waits for GOV01. A Git commit identity in this investigation is not represented as a downloaded archive's SHA-256.

## Adapter work an OCF selection would actually require

| Boundary | Required work / unresolved proof |
|---|---|
| Environment | Replace POSIX mutex/rwlock/semaphore/thread/timer/allocation primitives; audit blocking operations and IRQL. The [example POSIX environment](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/env/posix/ocf_env_default.h) includes GNU C constructs and an `atomic64` representation based on `long`; Windows LLP64 cannot be assumed compatible |
| Context data | [Context callbacks](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/inc/ocf_ctx.h) for allocate/free, lock/unlock, read/write/zero/copy/seek/erase need fixed ownership, alignment, length/error and IRQL rules; a user-mode `mlock` analogy is not an MDL/pool implementation |
| Storage | [Volume callbacks](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/inc/ocf_volume.h) for read/write/flush/discard/zero/metadata and geometry need real Windows status/count/FUA/ordering mapping; unsupported mutation paths cannot silently pass a cache-valid state |
| Queue | [Queue callbacks](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/inc/ocf_queue.h) distinguish deferred `kick` from permitted synchronous `kick_sync`; stop does not by itself prove cancellation and actual completion of device I/O |
| Callback lifetime | Keep request, source, slot and context alive through the last actual callback; match forwarding references and completion exactly once. `put`, cancel and metadata invalidation are not synonyms for hardware completion |
| C/Rust ABI | Opaque handles for ctx/cache/core/queue/volume/I/O; fixed-width direction/offset/length/status/count descriptors; callback tokens; no Rust references, `Vec`, `String`, enum discriminants or unwinding across C. The native OCF error-only callback is not the final product ABI |
| Allocation/telemetry | Count every environment allocation and high-watermark by class; include per-I/O buffers, backfill copies, queues and metadata, not only [metadata_footprint](https://github.com/Open-CAS/ocf/blob/94dcd9afc3b7dd2e9d52e25aeef61fc0b46fb4db/inc/ocf_cache.h). Prove failure and teardown behavior before a kernel budget |

These gaps make an immediate Windows port a larger unproven change than the original narrow policy. They do not imply that OCF cannot be ported. The upstream [quick start](https://open-cas.com/ocf/quick-start/) explicitly requires an adapter; it is not a standalone Windows cache.

## Memory and geometry decision

The spike uses 4 KiB lines, two slots and byte-granular fake transfers to expose overlap races. A byte transfer is a test abstraction, **not** permission for unaligned Windows I/O. Production line/sector pairs, buffer alignment, maximum request length and minimum supported cache/origin geometry remain D04/D05/B03 decisions. At minimum, compare 4/16/64 KiB cache lines against 512-byte logical/4 KiB physical and 4 KiB logical/physical devices; reject unknown or unsupported pairs instead of rounding origin mutations away.

Measured Linux x86-64 `Vec` capacities are recorded in [D03 evidence](../evidence/D03.md). They describe this host fixture, not OCF memory, peak RSS, allocator overhead or nonpaged pool. Fake media arrays must not become a RAM allocation proportional to the real HDD/SSD. The eventual product needs bounded metadata and in-flight buffers, not a copy of the entire origin in memory.

The design's hypothetical 32-byte entry illustrates the problem: 128 GiB / 4 KiB × 32 bytes = 1 GiB; with 64 KiB lines it is 64 MiB. These are arithmetic assumptions, not the spike's or OCF's actual entry sizes. Do not authorize use of a whole 128 GiB SSD until D04/B03 bound total metadata, queue, buffer and allocation-failure costs. A larger line saves index memory but can increase read amplification and contention; no policy/performance winner was measured here.

## Required next work and reconsideration

1. F09 uses the original Rust route and [engine contract](../contracts/engine.md), after all its existing dependencies are accepted. Preserve the four schedules and hand-derived expected outcomes, but drive a separate candidate against D07/F06 observations. Do not share freshness helpers with the oracle.
2. F06/F07 must cover the omitted parent/child fallback, retirement, persistence, flush/FUA, replay and evidence semantics. D03's small experiment does not close those tasks. F08/D10 remain necessary for Windows-mapped faults; the existing DAG is not silently relaxed.
3. W02 does not add an OCF FFI layer on the original Rust route. It still requires W01/D04/F09 and D09's reviewed boundary. If a narrow C/WDK shim is needed, it receives only the separately reviewed fixed ABI; this host experiment defines no kernel callback/IRQL guarantee.
4. Reconsider OCF only when an exact pinned adapter can run D02's schedules on OCF itself (including partial-applied errors), preserve lower status/count, bound allocations/cancel/rundown, select WI and volatile metadata explicitly, and satisfy licensing plus applicable Windows gates. Use at least OCF's supported minimum cache geometry; compare equal workloads/capacities before making performance or memory claims.
5. If original Rust fails the later independent oracle or exceeds the accepted memory budget, stop and revisit this choice. Passing a small author-written experiment is not sufficient reason to override a failing qualification result.

## Review / integration

Immutable executable source: `1d9833ce8ed7feaa6467fb56b48da4514ca090ad`. The earlier 12-test source `20115d499ad0b10ef3006c4561873db71a2f2f06` received independent Astra code review; two follow-up tests make its nonblocking coverage suggestions executable without changing the candidate implementation. The reviewer then accepted combined code/design commit `f0ff0e3d630ddb933f1419c231b4c4fa7d3a91a0`, rerunning 14 tests successfully. OCF factual research was separately crosschecked from pinned primary sources by SOL; the final Astra review did not claim a successful upstream refetch. GitHub commit creation for another task was interrupted; D03 has not been published or run in Windows CI. Do not mark D03 `done/accepted` or close #8 from this document alone.
