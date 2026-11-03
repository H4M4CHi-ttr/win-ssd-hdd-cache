# Coherence continuation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** D02の整合性契約を凍結し、Windows deviceなしで反例を再現できるportable検証を完成させる。

**Architecture:** trace入力、candidate model、原本の独立oracle、実Windows mappingを分離する。rootが実装し、Issue指定の独立reviewを受ける。実機が必要なD01/D04/D10/G1をoffline試験で代用しない。

**Tech Stack:** Rust 1.99.1 / edition 2024、既存serde/serde_json、unsafe禁止。実験checkerは既存cache-modelのhost test targetから読み込むpure Rust module。

**Spec:** [正式設計](../specs/2026-11-02-win-ssd-hdd-cache-design.md)、[契約](../../contracts.md)、[検証](../../validation.md)。

## Global Constraints

- HDDが唯一の原本。clean read cache / Write-Invalidate、write-backなし、persistent reuseなし。
- raw disk、Windows API、管理者操作、network、時刻/thread依存はportable modelに入れない。
- 失敗testの期待値を実装に合わせて変えない。mutantをparserが拒否するだけでは検出実績としない。
- D02受入までF05/D07/D03の実装を開始しない。各後続も既存の依存gateを守る。
- 既存の実行委任に従い技術選択をrootが行う。ライセンス、install/実機mutation、署名、製品公開の承認は別。

## Review Focus

- source-read途中でwriteが完了してから新しいtokenを取るstale-fill — D02 checkerとF07 fixture。
- cache readの物理leaseをparent timeoutで解放するlate completion — D02抽象counterexample、実Windows mappingはF08。
- completion成功と全line coverage/knownnessの混同 — D02とF05/F06 bounds/short-I/O tests。
- epochだけ進めてslotを再利用するABA — D02有限探索、D07 oracleの独立判定。
- flush成功を普通write完了と同一視する誤り — D07 durable frontierとF06 early-ACK mutant。

## Task 1 — D02 (#4): coherence contract + bounded checker

Files: `docs/adr/0013-coherence-model.md`, `docs/contracts/trace-v1.md`, `spec/trace-v1.schema.json`, `experiments/coherence/model.rs`, `experiments/coherence/README.md`, `crates/cache-model/tests/coherence_explorer.rs`。

Produces: event fields/limits、tokens/linearization/lease rules、`explore(Fault) -> Exploration`（bounded states/transitions/witness）、checked nonwrapping counter rules。これは製品engineではない。

- [x] 反例を先に固定。correct modelの全探索がviolationなし、invalidate-only/late-token/boolean-writer/short-fill/early-reuse/cancel-release/late-hit-check/wrapのmutantにwitnessを要求するtestを書く。
- [x] `cargo test -p cache-model --test coherence_explorer --locked --offline` をREDとして確認。
- [x] 最小finite transition modelと完全なtrace v1 schema/順序契約を実装。
- [x] 同command、fmt/clippy/planning、二OS CIを実行し、states/transitions/探索境界を記録。
- [x] 独立Astra review、修正再検証、backlog受入反映、PR/merge。
- [x] Issue本文/label同期、closeとreadback（PR #69後、22件の同期payload・body/labels/state等のreadback不一致0）。

## Task 2 — F05 (#14): strict parser

Files: `crates/cache-types/src/trace.rs`, `crates/cache-types/tests/trace_format.rs`, `tests/fixtures/trace-format/`。

Consumes: accepted D02 schema。Produces: `parse_trace(&str) -> Result<TraceV1, TraceError>`、event index/codeのみの固定error。coherenceを実行せず、型/上限/static参照を検査する。

- [x] valid events、unknown/duplicate、noncanonical numbers、exact/+1 limits、typed object-only、dangling referencesのRED tests。
- [x] 型付けparserを実装し、`cargo test -p cache-types --test trace_format --locked --offline`をGREEN（初回28、review修正後29 tests）。
- [x] 二OS CI・独立SOL以上review・受入/merge（PR #70、Windows129/Linux131、parser29件）。

## Task 3 — D07 (#17): independent oracle contract

Files: `docs/contracts/oracle-v1.md`, `spec/evidence-v1.schema.json`, `docs/adr/0017-test-oracle.md`。

Consumes: accepted D02。Produces: applied-byte history、known masks、volatile/durable、flush/FUA frontier、exactly-once result、reference/mutant evidence契約。

- [x] ordinary write後のpower-loss、failed partial mutation、discard、flush追越し、cache error/origin errorの小例を固定（手導出、未実行）。
- [x] candidateのgeneration/valid bitを信用しないoracleとevidence判定を定義。
- [x] 独立Astra reviewを通し、未観測のPASS化がないことを確認して仕様受入/merge（PR #71）。O01〜O18は手導出例で、F06の実行試験ではない。

## Task 4 — F06 (#20) / F07 (#23): replayと固定trace

Files: `crates/cache-model/src/{replay,oracle}.rs`, `crates/cache-model/tests/{replay_contract,coherence_traces}.rs`, `crates/cache-model/examples/replay.rs`, `tests/fixtures/coherence/`。Issueごとに別PR。

Consumes: D02/D07/F05 accepted。Produces: `run_trace(&TraceV1) -> ReplayResult`、immutable result/check observations、同seed/順序で同一のreport。

- [ ] read fill/hit、4095境界、two-writers、stale-fill、discard/rewrite、failed partial、flush、early-ACK/late-slot mutantのRED tests。
- [ ] 独立oracleとmodelを別moduleで実装。test期待値の根拠をcontractへリンク。
- [ ] 個別testと全workspace、二OS CI、独立Astra review、Issue受入/merge。

## Parallel lanes / stop boundary

D01は資料とlab handoffまで、D09は隔離build/ABI proofまで。D03はD02後にhost feasibility spikeと一次資料比較を行い、選定条件をreviewする。D01の実traceがない間はD04以降の製品kernel作業を開始しない。GUIや見かけの完成度のためにgateを飛ばさない。
