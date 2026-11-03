# ADR-018: C/WDK observation lifecycle, Rust collection and audit

2026-11-03 JST. **Independently accepted P04 source/build boundary; implementation
waits for the planning checkpoint's publication/readiness.** Proposed-content
SHA-256 `04f862cfdff29b3aabb5c705dbbc519f4e929f181ef0ddac974873acdcf0008e`.
Related: [P04 #89](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/89),
[ADR-007](0007-rust-kernel-boundary.md), [P02 evidence](../evidence/P02.md).

## Decision and scope

Use C17/WDK for the complete, narrowly scoped P04 observation driver: native
dispatch/completion/PnP/control lifetime and an in-place finite metadata recorder.
Keep native transport validation, archive reconciliation, conservative offline
projection and the collector in Rust. The product model/cache engine, service,
CLI and future Tauri backend remain Rust. This is a reviewed exception to
ADR-007's no-unreviewed-C-ownership-growth rule, not a production driver language
decision or an adoption of the candidate interception position.

The source unit will never be installed/loaded by this task. It cannot provide
cache data, originate storage I/O, format a device, or become a product release.
Its control endpoint only returns bounded, payload-free metadata. Native C
remains memory-unsafe; choosing it does not make OS lifetime rules automatic.

## Why this exception

P02's safe Rust producer has private native-layout tickets and a large by-value
constructor. Connecting it directly to kernel callbacks would additionally need
a stable nonpaged storage/initialization ABI, context-token bridge and reviewed
IRQL/aliasing/unwind/stack contracts. A host optimizer result cannot establish
those requirements. D09 proved a much smaller build-only bridge, not this live
ownership bridge.

For a finite observation experiment, a single C/WDK lifecycle layer avoids that
extra ABI while leaving the larger collection/validation work in Rust. It uses
the already reviewed P02 scalar extraction rules, but **does not execute the Rust
Producer in kernel**. The C recorder has its own fixed literal expectations and
tests; agreement with the Rust producer is additional evidence, not its only
oracle. No per-I/O allocation or large recorder-by-value copy is allowed.

| Alternative | Consequence |
|---|---|
| New live Rust/C producer bridge | Viable future work, but adds an independent nonpaged-storage and callback ABI before useful observation evidence exists |
| Move the whole project to C | Rejected: unnecessary and contrary to the owner's Rust-first direction |
| User-mode producer assigns identities after transport | Rejected: can conceal kernel-side event loss and requires duplicate tracking |
| Add only another scalar helper | Rejected as the next handoff: would still leave entry, lifetime, transport and collection unimplemented |

## Required controls and revisit

- Fixed contract before implementation; actual C source under isolated host DDI
  scheduling tests, not a second implementation of the lifecycle.
- Independent review of native lifetimes, lock order, IRQL/residency obligations,
  exact target selection, failure paths, wire values and collector FFI.
- Release-only completion when not recording; stopping or filling the log never
  substitutes for real IRP completion. REMOVE waits on actual remove holds.
- Linux/Windows host tests and pinned WDK compile/link are separate evidence.
  Fake headers must be absent from the real WDK build. Neither qualifies runtime.
- No INF/catalog/install/load, security-setting changes or binary upload. All
  D01/D04/D10/F08/F09/G1 and owner decisions retain their existing state.

Revisit if a feature needs storage payload ownership, DMA/MDL mapping, actual
cache I/O, multi-target operation, kernel Rust producer integration or product
deployment. Do not extend this exception to those features by convenience.
The exact experimental contract and its independent review remain the authority
for P04 source work; this ADR alone does not make a blocked Issue ready.
