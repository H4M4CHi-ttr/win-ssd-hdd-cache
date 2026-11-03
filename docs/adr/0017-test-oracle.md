# ADR-017: Independent lower-history oracle

Status: D07 specification independently accepted at `cefd96ea936e664c648ba67437a233261a58a92b`; remote publication/integration pending. 2026-11-02. Issue [#17](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/17).

## Reviewed decision

Use a bounded, pure Rust lower-history oracle separate from the candidate's coherence implementation. The normative [oracle-v1 contract](../contracts/oracle-v1.md) consumes real fake-backend transfer/completion/persistence observations and separately checks candidate claims. Cache generation/valid bits cannot supply oracle truth. Parent admission, actual effects, actual completion, caller commit and durability are distinct events.

Do not compare every returned origin read to the latest entire origin: fragment sampling may legitimately straddle a concurrent write. Track each sample and its known mask. For a cache result/publication, independently verify fill lineage, overlapping mutation admissions, slot write provenance and actual outstanding leases. Do not reuse the reference policy's freshness helpers in the oracle.

Use fixed hand-derived examples O01–O18 and separate schema/error/backend/safety/assertion outcomes. The three required mutants must reach the independent oracle and produce their named safety violation. A malformed trace, missing test or assertion-only mismatch is not mutant-detection evidence.

## Alternatives considered

- Candidate-versus-itself output comparison: rejected; common bugs can pass.
- Latest-origin byte equality only: rejected; false positives for fragmented concurrent reads and false negatives for same-valued stale lineage.
- Treat every completion as durable: rejected; confuses volatile device cache with persisted history.
- Read-cache-is-safe assumption: rejected; stale filesystem metadata can lead the filesystem to issue destructive origin writes.
- Wait for real Windows hardware before any model testing: rejected; portable ownership/lineage cases can be bounded and replayed now, but cannot qualify hardware.

## Consequences

The oracle costs additional bounded history and explicitly separates unknown, pending, abandoned and unobserved. This is intentional. `evidence-v1` reports deterministic counts and fixed codes without payload/path disclosure. No-shadow support is shown as not_applicable, not a fabricated pass. D07 is a specification; actual mutation tests, replay, fixtures and CLI are F06/F07. D10 separately maps these observations onto Windows ownership/fault injection.

Acceptance of D07 does not change D02 semantics, select a production kernel attachment, or approve installation, raw I/O, signing, licensing or public binaries.

## Independent review and limits

An independent Astra reviewer accepted immutable specification commit `cefd96ea936e664c648ba67437a233261a58a92b`. Review fixes made stage precedence explicit (input lifecycle, lower promise, candidate safety, assertion) and added a separately counted post-event retained-result guard, including late I/O after caller commitment. Category counts, meaningful checks, unobserved/inconclusive outcomes and provenance are normative, not inferred from implementation output.

O01–O18 are hand-derived contract examples, **not executed tests**. The evidence schema was checked for JSON syntax; no claim of full JSON-Schema validation is made. The schema is shape-only and requires the runtime semantic checks specified in the contract. Local planning validation passed (28 labels, 52 tasks, 42 documents, 99 relative links) before this acceptance record. No F06/F07 implementation, Windows execution or hardware test was performed in D07.

GitHub publication is unconfirmed after interrupted commit-creation calls. This document does not imply main contains the specification or that #17 was closed. Keep the repository's integration snapshot unchanged until the accepted artifact is actually published and integrated. F06 also requires F05's outstanding Windows host CI and acceptance.

## 2026-11-03 公開・統合の追記

[Phase 2統合記録](../evidence/2026-11-03-phase2-integration.md)にmain統合、今回のCIと受入を記録。上記のlocal/未公開という記述は当時の履歴であり、今回解消した。runtime/実機・製品受入の未実施は変わらない。
