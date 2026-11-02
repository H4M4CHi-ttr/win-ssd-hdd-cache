# Host dependencies

2026-11-02. These are ordinary Cargo dependencies, not vendored source. This inventory does not grant a license for this repository; GOV01 remains pending. Do not copy third-party source/license terms into the original-code policy without its separate decision.

Direct exact pins: serde 1.0.229 (derive; typed structural decoding), serde_json 1.0.154 (JSON syntax; arbitrary_precision preserves numeric-token syntax independently of f64 representability). Official package metadata was checked with cargo info; sources are [Serde](https://github.com/serde-rs/serde) and [serde_json](https://github.com/serde-rs/json). Object-only, unsigned-token and required-nullable adapters are original small safe-Rust wrappers around serde, not a custom JSON tokenizer. Raw Value roundtrips are not used to erase duplicate keys.

| Package | Locked version | Metadata license |
|---|---|---|
| serde / serde_core / serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 (`arbitrary_precision`, `raw_value`) | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| zmij | 1.0.23 | MIT |
| pulldown-cmark | 0.13.6 (default features disabled) | MIT |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| unicase | 2.10.0 | MIT OR Apache-2.0 |

All 11 downloaded registry archive SHA-256 hashes were independently compared with Cargo.lock: 11/11 match. The package license notices remain in the downloaded dependencies. Distribution/notice assembly and original repository licensing remain GOV01/C08 responsibilities. This is not a complete legal or vulnerability audit. Cargo.lock and --locked constrain transitive versions; --offline is used after acquisition for local host tests. Our host crates forbid unsafe; that is not a claim that every upstream dependency contains no unsafe implementation.

F02 adds exactly pinned [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) 0.13.6, without optional HTML/getopts features, for CommonMark Link/Image events. Its two added transitives and all existing packages were checked via locked Cargo metadata. All 14 downloaded archive SHA-256 values match the final lockfile (14/14). serde_json's raw_value feature supports borrowed raw JSON fragments for duplicate-key/token-preserving planning validation; it adds no package and does not change the public product codec contracts. No upstream source is vendored.
