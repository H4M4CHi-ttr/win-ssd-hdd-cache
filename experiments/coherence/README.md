# D02 finite coherence experiment

これはsafe Rustの**有限な反例探索器**。製品cache engine、実I/O runner、Windows driverではない。trace v1 parser/replayはF05/F06で別に実装する。model.rsをhost integration testから読み込むため、本体crateの製品APIには追加しない。

## 実行

```sh
cargo test -p cache-model --test coherence_explorer --locked -- --nocapture
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo xtask check-planning
```

Rust 1.99.1。新規dependencyなし、unsafe/OS API/network/実deviceなし。test-firstの初回にはmissing model moduleでFAIL、その後実装して3 tests PASS。レビュー後の二つのfill反例もmissing caseでREDを確認し、追加後は5 tests PASS。単なるsyntax typoの失敗は機能のRED証拠に数えない。

## 探索範囲

各scenarioは最大2byteセル、2writer、2fill、1hit、1physical slot、1late child、最大18step。writer適用と第一source sampleは1byteずつ。追加した二つのfill反例では、第二sourceはwriter完了後に固定順序でcaptureする。3本以下のprogram-order chainの全interleavingをbreadth-firstに探索し、`(全state, chain positions)`で重複除去する。HashSetの反復順は使わず、同じ入力で最短step数の同じwitnessを得る。

探索上限100,000 states/case。上限に達したらINCONCLUSIVE、空探索/未完了をPASSにしない。故障caseは最初の安全違反で停止し、全探索を終えたとは表示しない。正常caseだけ全terminal stateまで探索する。

fill等のadmissionが拒否された後のchain continuationはskipであり、未存在I/Oをdeviceへ発行する意味ではない。mutantをparser errorで排除せず、同じ入力programで不正publish/commit/physical accessが起きることを検出する。

## ローカル実測（2026-11-02、Linux）

| 正常case | unique states | transitions | terminal states |
|---|---:|---:|---:|
| StaleFill | 130 | 199 | 6 |
| TwoWriters | 2172 | 4505 | 52 |
| PartialWrite | 82 | 122 | 4 |
| ShortFill | 8 | 7 | 1 |
| UnknownFill | 9 | 8 | 1 |
| HitRace | 54 | 83 | 3 |
| CounterExhaustion | 408 | 671 | 13 |
| LateSlotWrite | 16 | 19 | 3 |
| LateSlotRead | 16 | 19 | 3 |
| CancelledIo | 16 | 19 | 3 |
| LateCallerCompletion | 8 | 9 | 1 |
| StaleOverwrite | 19 | 18 | 1 |
| SupersededPublish | 19 | 18 | 1 |

case別合計2,957 states / 5,697 transitions / 92 terminal states。異なるscenario間の同じ形のstateを同一視した集計ではない。Windows CIの実測結果はPR evidenceを参照し、上表から推測しない。

## mutation tests / witnesses

14 case/fault組すべてで安全違反を検出する。

- invalidate-only: Aをsource sample → B write完了 → A publish。
- late token capture: A sample後にwriterをadmit → fill submitで新generationを取得 → B apply完了 → A publish。
- boolean writer: 2writerをadmit → 一方だけcomplete → fillを開始/公開、実際にはもう一方が生存。
- partial write effects: 一部byteだけ変更してcomplete → 古いfull-line fillを公開。最終statusをstateに持たないため成功/失敗の伝播を試験したとはしない。この抽象はどちらのstatusでも残り得る部分効果だけを表す。status保持はD07/F06で別に検証する。
- short fill: 一方のbyteしかslotへ転送していないのにfull entryを公開。
- unknown fill: unknown byteを含むsourceからentryを公開。
- no hit revalidation: cacheをsample/complete → writer admission → 古いhitをcommit。
- generation wrap: counter上限2で旧token=1を保持、2回のwriteで1に戻り、旧fillを公開。
- epoch lease release: old physical I/O開始 → epoch invalidate → rebind → old transfer。
- cancel lease release: old I/O開始 → cancel request → rebind → old transfer。
- double completion: cache timeout → origin fallback commit → late cache callbackがcallerを再complete。
- stale physical overwrite: A sourceを保持 → originをBへ変更 → 第二fillがBを公開 → 旧Aのwrite admissionがvisibilityを残す → Aでslot上書き → actual complete後、Bのvalid metadataでAが可視になる。
- superseded publication: 第二fillのB write完了 → 旧Aの別writeがslotを上書き → Bのpublicationが現在のcontent provenanceを無視すると、古いmetadataでAを公開する。

最後の二つは固定18stepの反例再現で、並行する任意数のfillを網羅したとはしない。incarnationはこの有限program内で最大3、slot counter枯渇の全検証は後続replayの責務。visibilityの独立検査はexclusive physical write中ではなく、実完了後にread可能となった点で行う。

slot-read/slot-writeの2caseは**同じphysical lifetime抽象**を使い、実際のread/write transfer差や複数shared-readerは表現していない。これを二種類のWindows実証と数えない。hitの最短witnessがwriter適用前でも、writer数条件という明示安全契約の違反を検出しており、すべてを実bytes破損の実測とは表示しない。

## 検出していないこと

全cacheline geometry（F01は別試験）、任意writer/slot数、full trace v1、durability/FUA（D07/F06）、cache corruption、allocator/IRQL/IRP/MDL/DMA、持続的device hang、hardware power loss、実cache性能。永続hangを有限回のcancelで解決できる証明ではない。

[coherence ADR](../../docs/adr/0013-coherence-model.md)と[trace契約](../../docs/contracts/trace-v1.md)の全要件を、このtiny checker一つで証明したとは主張しない。後続harnessは独立oracleで範囲を広げ、kernel mappingはD04/D10/F08で別の証拠を要求する。
