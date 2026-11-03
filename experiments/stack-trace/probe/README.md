# P04 capture-only candidate

This is experimental observer/collector **source**, not an SSD cache, installer,
or permission to attach a filter. No cache hits, fills, SSD writes or originated
storage requests are implemented here. The existing HDD is not modified by a
test. Local host-memory checks exist; consult the
[evidence](../../../docs/evidence/P04.md) for actual independent review and hosted
build status, rather than inferring success from the presence of these files.

The primary implementation remains Rust. The complete but narrowly scoped C17
WDK observer is the justified exception in
[ADR018](../../../docs/adr/0018-capture-probe-language-boundary.md).
Its [contract](../../../docs/contracts/upper-candidate-probe-v1.md) is normative.

## Boundaries

| Component | Responsibility | Not established by this component |
|---|---|---|
| `common/wire.c`, `common/recorder.c` | Fixed little-endian frames;64 contexts/256 records; checked loss counters | Windows callback lifetime or capture coverage |
| `wdk/driver.c` | Exact-target optional attachment, bounded scalar observations, remove holds, PnP/power and FileObject-bound control | Safe deployment, correct real stack position, actual Windows runtime |
| `host/` | Same driver TU with isolated fake DDIs, guard pages and deterministic thread barriers | Real IRQL, PnP sequencing, hardware or HVCI qualification |
| Rust `cache-probe-core` | Strict archives, retained-history checks and conservative P03 v2 projection | Authenticity, authority, target address space, complete I/O coverage |
| Rust `cache-probe-collector` | Bounded offline CLI; compile-time and command-line gated fixed-endpoint transport | Permission to run live capture |
| `tools/build.ps1` | Hosted-only source/hash/include verification and never-loaded WDK link | Installation, registration, attachment or loading |

An unknown Information value is not zero. Missing replies are not fabricated
producer drops. Archive terminal receipt is not capture qualification; every
projection keeps unverified provenance and boundaries explicit. A valid emitted
report/projection returns exit4, not an operating-success endorsement.

## Safe host verification

From the repository root, with the pinned ordinary Rust toolchain, Node and a C17
compiler (GCC on Linux; VS amd64 developer shell for the Windows host harness):

```sh
cargo test -p cache-probe-core -p cache-probe-collector --locked
cargo test -p cache-probe-core -p cache-probe-collector --release --locked
node experiments/stack-trace/probe/tools/test-host.mjs --suite all
node --test experiments/stack-trace/probe/tools/test-build-contract.mjs
node experiments/rust-wdk/tools/test-pe.mjs
```

These commands use memory, fixed synthetic files and fake transports. They do
not open the observer control device. The Linux feature-enabled collector build
does not compile the Windows FFI module and must not be cited as Windows proof.

The [P04 workflow](../../../.github/workflows/probe-build-proof.yml) runs ordinary
hosted Linux and Windows jobs with read-only repository permissions. On Windows,
it tests the fake-DDI source, compiles/links the explicitly enabled Win32 collector
without invoking it, verifies the pinned Microsoft package hashes/signatures,
and builds the actual four kernel TUs without fake headers. The image stays in
the disposable runner; only text/JSON evidence is uploaded. No INF, catalog,
registration, boot-setting change, signing or loading step is provided.

## Before real Windows work

Use the [read-only inventory handoff](../../../docs/handoff/2026-11-03-windows-next-step.md)
on a specifically designated environment first. Device identity, effective ACL,
special-use exclusions, attachment position, approved workload and recovery must
be established separately. The selector and live command-line switch are guards,
not authorization. The deliberate nonzero-InPath usage veto is not blanket I/O
transparency; it does not make attachment to an existing special-use path safe.

The product driver, cache-engine integration, service, Tauri GUI, signing,
installation, performance and runtime qualification remain separate work. No
D01/D04/D10/F08/F09/G1 or owner approval follows from a successful source build.
