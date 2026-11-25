# ABORT-1 launcher and synthetic Windows IPC

This crate is default-off. It supplies a fixed sibling-only launcher for
`cache-probe-collector.exe`; it has no executable-path, device, IOCTL, elevation,
registration, or package-install option. The sibling image must belong to the
trusted deployment. The launcher pins its file identity against write/delete
replacement and checks the suspended child's mapped image identity before
resuming it. This does not authenticate an independently started collector's
ambient environment or certify a deployment/signature.

`experimental-live` compiles the production launcher. Its sole CLI is
`--duration-ms N` (canonical decimal 1 through 30000). Operator input byte `a`
issues the fresh per-attempt unnamed manual-reset event. A separate bounded
recording channel emits fixed JSON evidence for request publication or failure. Neither is a STOP ACK.
The binary child output, child diagnostics and notification reader are
independent. A blocked destination may block its own pipe, so an external host
monitor still has to classify unconfirmed completion. Never execute this
production target without the separate lab authorization.

Only the event's SYNCHRONIZE duplicate and the explicit stdin/stdout/stderr pipe
handles enter STARTUPINFOEX's handle list. The child receives a newly constructed
environment, not the parent environment. The accepted handle has exact
SYNCHRONIZE rights, OS Event type, exactly the initial inheritance flag with no protected-close/unknown flags, and is cleared for
onward inheritance before any device open. Rejected unknown handles are not
waited, adopted or closed. Acceptance does not prove who created an event;
the trusted launcher path is an operational prerequisite.

`synthetic-ipc` builds `p04-ipc-worker`, which has no dependency on device
transport or registration APIs. The same event-validation capsule and Win32
spawn code are used. Tests cover malformed startup values, stale handles,
wrong type/excess rights, non-pipe stdout, denied child mutation, excluded
inheritable sentinels and sender rights, two distinct attempts, duplicate
notification, close/inheritance flags, and binary pipe bytes. One test also
starts a synthetic sender process, transfers its worker observation handles
privately to the supervisor, waits for the sender process to exit without
SetEvent, then verifies the worker remains unnotified. The temporary numeric
handle packet remains inside its binary control pipe and is not a report.

On an SDK-equipped Windows AMD64 process with the workspace's exact toolchain:

```
cargo check -p cache-probe-launcher --all-targets --all-features --locked
cargo clippy -p cache-probe-launcher --all-targets --all-features --locked -- -D warnings
cargo build -p cache-probe-launcher --bin cache-probe-launcher --features experimental-live --locked
cargo build -p cache-probe-launcher --bin p04-ipc-worker --features synthetic-ipc --locked
cargo test -p cache-probe-launcher --lib --features synthetic-ipc --locked
cargo test -p cache-probe-launcher --test windows_ipc --features synthetic-ipc --locked
```

The first production build is compile/link only. Run only the synthetic tests;
none open a CDO, enumerate a device, modify a registry/service/Driver Store, or
install/load a driver. A successful synthetic test is not Windows storage
qualification. The independently maintained `supervision` module records
external monitoring facts; its tests do not prove an actual host watchdog.

The guest-side supervisor uses its own clock and process handle, polls once per
second, and records a sticky 15-second deadline if process exit, pipe EOF,
received SHA-256, or forwarding/flush remains unconfirmed. It never terminates
a running production child. Later observations retain their later times without
rewriting that deadline. This implements a guest-side observation source;
it does not prove that an independent external host watchdog works.

Private stderr JSON schema `p04_guest_supervision_v1` carries a fresh anonymous
128-bit `trial_id`, independent of the device session/wire. Each child gets its
own pipes; the launcher's trial envelope binds its pipe observations. Fields
include actual source `request_published_at_ms`, observed process-exit and EOF
times, `received_bytes`, lowercase `received_sha256` (only after real pipe EOF),
`stdout_forwarding_complete` (only after write/flush), and sticky deadline and
monitor-loss times. Unknown values remain `null`; `saved` and
`receiver_hash_authenticated` stay `null` because this launcher has no external
receiver persistence/authentication proof. A received hash does not prove that
a forwarded archive was saved. A bounded queue isolates stderr backpressure;
success requires acknowledged evidence flush within one second. Failure exits
3 without attempting another blocking diagnostic write.

Handle validation first walks a bounded handles-only snapshot of the current
process. This avoids probing absent raw handles under permanently enabled
StrictHandleChecks. Snapshot/marker failure or the 4096-entry bound fails closed.
Only a present inherited handle is queried for Event type and exact rights.
The startup owner must not concurrently close/reuse that inherited handle;
the snapshot does not authenticate event origin. Synthetic exclusion assertions
compare object identity when a numeric slot has been reused for an unrelated
runtime handle. No mitigation policy is weakened.

Raw startup adoption is an explicitly unsafe ownership boundary. Its caller
must establish exclusive transferred ownership and prevent concurrent startup
environment mutation/handle close/reuse; the collector invokes it only from its
private Win32 capsule before background work. Public safe APIs cannot adopt an
arbitrary caller-owned event from a mutable environment. The synthetic local
non-inheritable decoy keeps its original Rust owner after rejection; the test
checks no adoption/wait and then closes it through that original owner.

All launcher stderr records now pass through one bounded queue and one writer.
The writer holds the stderr lock through each complete JSON line and its flush;
raw child diagnostic chunks are never copied directly to stderr. The child pipe
reader assembles LF-terminated records across arbitrary read fragmentation. It
accepts only the exact `p04-controlled-v1` field/type/reason allowlist and fixed
collector error codes, then emits `p04_child_diagnostic_v1` envelopes with the
same anonymous `trial_id`. Unknown text is never echoed, including source paths.

The diagnostic framing budget is four records, 4096 bytes per line, and 64 KiB
received bytes. A valid final EOF emits `diagnostic_eof` with `complete:true`
only when every line was recognized and queued and no unterminated tail remains.
This states diagnostic stream completeness, not STOP acknowledgment or saved
archive evidence. Truncated EOF, extra/duplicate fields, malformed UTF-8/JSON,
unknown/private text, overflow or read failure emits only a fixed redacted
`diagnostic_failure` with `complete:false`, marks evidence failed and drains the
remaining pipe. Queue/sink failure also prevents success; the binary stdout
receiver and notification/monitor paths never wait on the diagnostic sink.
Partial sink writes remain incomplete and never receive a fabricated success
suffix. Normal completion still requires acknowledged flush of every queued
record; destination persistence remains unknown.

After both readers start, forwarding always joins both before propagating any
binary destination, child wait/exit-query, or diagnostic error. This lets the
already-running diagnostic producer queue its final recognized record and EOF
before the launcher's evidence flush is evaluated. A failed binary destination
still retains received bytes/SHA-256 and EOF facts, with forwarding incomplete
and persistence unknown. A failed process wait with a still-running child can
leave readers pending; notification and guest monitoring remain independent,
and no termination or completion is inferred.

If the diagnostic reader cannot start after the binary reader has started, the
launcher marks diagnostics unavailable/incomplete, queues a fixed redacted
failure if possible, and joins that existing binary reader before returning the
spawn error. It starts no replacement thread and does not force-close or kill
the child. The independent watcher and notification source stay available while
that reader remains pending.
