# Pure registration model

This crate implements REG-1 through REG-4 without dependencies, unsafe code,
Windows APIs, network access, or a pluggable mutation backend.

`make_plan` binds an immutable plan to one coherent inventory, an independently
supplied protected-OS/target record, the exact reviewed package manifest, the
attempt, target object and enumeration generation, original properties and
expected values. Disk numbers, labels, capacity and shared hardware IDs do not
select an instance. Missing filter properties, valid empty `REG_MULTI_SZ`, invalid
data and failed queries remain distinct. All protected/device/class filter
baselines must be readable. Observed OS pagefile-location differences are kept
with the plan; target special use or unknown use rejects planning.

After strong identity selection, T1 must have a known capacity of exactly 8 GiB.
Other or unknown capacities cannot select a substitute device. Expected class,
stack entries and the observed function service must be nonempty; the function
service must occur in the expected stack.

Plans contain private evidence. Their `Debug` output and public outcomes are
redacted; inventories and private journal bytes are private comparison records,
not public diagnostics. `Journal::from_private_bytes` requires the original exact
bound plan, rejects another binding/truncation, and never resumes effects.

`synthetic::SyntheticLab` is a concrete memory store, not an interface that can be
implemented by Windows code. P/S/K/F each recheck the independent protection
record, rights, whole-lab exclusion epoch, target generation, complete baseline,
and preceding expected effects. Call targets and actual store effects are logged
independently from intent/return/confirmation/completion records. V verifies the
whole expected snapshot; it performs no mutation. Foreign V changes retain their
own actor and target. An API error cannot confer success even when its effect is
readable. No rollback, old-list overwrite, service deletion, load or PnP action
exists.

The decision compares the returned snapshot to both bound before/after states;
it never receives the fake store's hidden effect flag. Only a successful return
plus an independently matching after-state can confirm a stage. A failed return
with a readable after-state retains that observation and stops uncertain. A
successful return with a before-state is contradictory and cannot confirm.

`SyntheticLab::restart` and `restart_read_only` cannot issue effects, including
when the private record is missing. `apply` on an already started lab performs
only that reconciliation. Only an exact completed record plus fresh matching
inventory/protection can yield `AlreadyConfirmed`. Similar current values alone
do not grant ownership. All six real qualification flags remain false.

`Plan::real_apply_gate` unconditionally returns `RealApplyUnavailable`. Neither
synthetic rights/exclusion, a reviewed manifest, a feature, nor a caller-defined
backend can bypass it. Package inspection and production eligibility are separate:
the Windows crate supplies the inspected manifest, while the source/build proposal
has no accepted signed deployment package or real exclusion capability.

Admission and comparison require printable ASCII excluding comma, with at most
199 characters. Both protected IDs and every enumerated ID are checked before
selection/collision detection, so a non-ASCII alias cannot silently appear distinct.
This conservatively follows the [PnP character restrictions](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/irp-mn-query-id)
and [device instance ID length limit](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/device-instance-ids).
The selector codec separately preserves exact Unicode and up to 511 UTF-16 units;
codec validity does not confer eligibility. Matching ignores ASCII case only and
never applies Unicode expansion, trim, wildcard or prefix matching. Windows
ordinal comparison belongs to the compile/link-only boundary and cannot widen
this model's gate.

## Synthetic checks

The named `r01`–`r14` tests include every R13a–d × P/S/K/F expansion, including
both API-error effects and every readback-fault kind. The oracle fixes A/T/U and
class targets and values independently of the engine. R07 deliberately mutates
successful call traces to prove that forbidden targets are detected even when
the final state looks correct. R08 demonstrates why an overwrite can lose a
competitor entry while its own readback matches.

Eleven further tests start the same test executable in a strictly synthetic child,
persist separate journal/store snapshots, block at the exact pre-call/post-effect
or V/completion cutpoint, and terminate only that child. They reload the exact
private record, construct a fresh store observation, and assert no restart writes.
Harness startup failure, timeout or failed child cleanup is a test failure.

```text
cargo fmt -p cache-probe-registration -- --check
cargo clippy -p cache-probe-registration --all-targets --offline --locked -- -D warnings
cargo test -p cache-probe-registration --offline --locked
cargo test -p cache-probe-registration --release --offline --locked
```

Synthetic results prove model behavior only. They do not prove Driver Store,
services, real registry exclusion, signing, installation, attachment, driver
load, storage I/O, or runtime/hardware qualification.
