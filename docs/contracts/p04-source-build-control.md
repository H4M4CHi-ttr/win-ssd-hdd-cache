# P04 controlled capture and registration proof

This is the local source/build extension to the [v1 probe contract](upper-candidate-probe-v1.md). It adds a user-mode owner cancellation path and an experimental registration model. The wire ABI, v1 archive/footer, previous CLI modes, driver, and production gates retain their original semantics.

## Registration

`cache-probe-registration` resolves the protected OS disk and the one target from independent identity/usage evidence. A dry run binds the planned package, inventory generation, original filter values and expected changes. Its synthetic executor records each attempted and applied P (package), S (demand service), K (exact selector), F (instance filter) action. V is verification only. A failed call can already have taken effect. A lost response, unproven exclusion or interrupted journal does not authorize a retry or restoring an old multi-string.

The [Windows boundary](../../crates/cache-probe-registration-win/README.md) compiles real SetupAPI and registry/service access with faked leaves for tests. The linked proof executable retains those paths for static inspection. No public API or CLI accepts an apply token for a real devnode. The narrow service-only INF proposal has no deployable signed bundle. The feature is not a Windows installer. The [model notes](../../crates/cache-probe-registration/README.md) describe the synthetic authority and recovery journal.

## Cooperative cancellation

The additional `collect-controlled` mode requires an AMD64 Windows experimental build, the explicit live opt-in, a valid 1–30000 ms duration, a binary stdout pipe and the private per-child launch descriptor. The [launcher](../../crates/cache-probe-launcher/README.md) creates an unnamed, initially nonsignaled manual-reset event for each trial. It retains signaling authority and uses an explicit handle inheritance list. The collector accepts a synchronize-only event after validating input and OS-reported properties, before opening its fixed CDO. A numeric handle does not authenticate a launcher. The separate deployment/trust boundary remains required.

The same capture owner processes returned frames and latches the first stop cause plus concurrent causes. Cancellation observed before BEGIN exits 5 without an archive. After a valid BEGIN the owner stops normal DRAIN, uses its original FileObject for STOP and bounded recovery, and preserves the existing uncertainty/cleanup budget. No second control owner, replayed destructive DRAIN, fabricated success footer, or new wire fault bit is introduced. A valid emitted archive still exits 4 and does not prove persistence. Fixed diagnostics and the receiver's byte count/SHA-256 are separate from native stdout.

The 5,000 ms post-STOP poll limit runs between returned calls. The 15,000 ms observation threshold for the reviewed 1,000 ms trial is measured outside the collector from launch. Neither interrupts a non-returning synchronous call. Process exit, STOP ACK, terminal frame, EOF, forwarding and independently confirmed persistence are different facts. The guest monitoring implementation does not qualify an external host watchdog. No fault path forces a live guest power-off or reverts a snapshot.

## Reproducing synthetic proof

- Run the ordinary workspace format, all-target Clippy, tests and planning checks from [development](../development.md).
- Run the release portable tests with the default features for the probe core, collector, registration model, Windows registration boundary and launcher. Preserve libtest named results and H scenario records with `-- --show-output --test-threads=1`.
- On Windows AMD64 also run the real user-mode IPC suite against its synthetic worker. Compile **and link** the gated collector, gated launcher, and registration link proof. Verify their import thunks without running the production binaries.
- The [P04 workflow](../../.github/workflows/probe-build-proof.yml) specifies the exact future CI commands. The [case manifest](../../experiments/stack-trace/probe/tools/matrix-cases.json) and [matrix verifier](../../experiments/stack-trace/probe/tools/matrix-proof.mjs) distinguish required case names and H point/outcome records from total harness passes.

The source/build checks establish neither real exclusion from another Windows installer nor device-stack order, effective control ACL, signature acceptance, or recovery from a hung guest. Real registration, loading, capture, signing and machine changes are outside this proof.
