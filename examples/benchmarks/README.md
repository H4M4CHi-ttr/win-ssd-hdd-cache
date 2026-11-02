# Offline benchmark-report examples

These are **fictional fixtures**, not measurements of this project, an HDD, an SSD, Windows, or any cache implementation. A displayed speedup of 1.250000 is an arithmetic test value, not a performance claim.

From the repository root:

```sh
cargo run -p cache-cli -- benchmark-report --base examples/benchmarks/base.json --candidate examples/benchmarks/candidate.json --format text
cargo run -p cache-cli -- benchmark-report --base examples/benchmarks/base.json --candidate examples/benchmarks/candidate.json --format json
```

The literal outputs are [comparison.txt](comparison.txt) and [comparison.json](comparison.json). The [offline format contract](../../docs/contracts/host-formats-v1.md) specifies eligibility, ordering and non-reflecting errors. Matching identifiers and metadata are necessary for comparison, but the tool cannot authenticate measurement provenance or prove that the runs were experimentally equivalent. It does not perform benchmarks, adjust the OS cache, contact a service or open devices.

Both inputs must be bounded ordinary files in cooperatively stable directories. The reader is unprivileged convenience code, not a race-proof path sandbox for a privileged service. Mismatched workload/environment/toolchain/working-set/cache-condition/repetition metadata is refused; treatment differences remain visible. Unknown metrics stay unknown.
