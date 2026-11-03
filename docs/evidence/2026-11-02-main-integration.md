# Main integration and D02 acceptance

更新: 2026-11-02 JST。これはsource/host成果物の証拠であり、Windows driverの実行・製品安全性・実機qualificationの合格証明ではない。

## 1. 統合したhost基盤

PR #54〜#65を依存順に、expected headを指定した通常mergeで統合した。force pushや保護回避は行っていない。最終merge commitは `d7f94a01e85f70d35641cd91127bbcc26ad78b07`、treeは `63ae1a0fc36fd6aa51c82ccb62cd2b3b4a54f0fd`。このtreeは[統合前に受入済みのsource snapshot](2026-11-02-host-foundation.md)と一致した。

main pushの[host CI run 41460366605](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41460366605)もsuccessを読み戻した。過去の97/95件の試験履歴は元記録に保持し、後続の試験件数に置き換えない。

| ID | Issue | 統合PR |
|---|---|---|
| F01 | [#5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) | [#54](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/54) |
| F03 | [#7](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/7) | [#55](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/55) |
| AUD01 | [#51](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/51) | [#56](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/56), [#65](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/65) |
| F04 | [#12](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/12) | [#57](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/57) |
| C01 | [#16](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/16) | [#58](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/58) |
| B01 | [#18](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/18) | [#59](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/59) |
| C03 | [#21](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/21) | [#60](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/60) |
| C02 | [#22](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/22) | [#61](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/61) |
| F02 | [#10](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/10) | [#62](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/62) |
| B02 | [#28](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/28) | [#63](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/63) |
| F10 | [#13](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/13) | [#64](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/64) |
| D02 | [#4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4) | [#67](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/67) |

これら12課題はartifactの独立受入・main統合を満たした。GitHub Issueの本文同期・close/readbackは別操作であり、この文書だけで完了したと扱わない。Issue側に最終状態と証拠を記録する。

## 2. D01は資料だけ統合、全体は未受入

[PR #66](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/66)、merge `f1e8f75bc7ff032481f2682e4542e728cce884f9`。対象は[ADR-004](../adr/0004-interception-boundary.md)、[I/O対応表](../contracts/io-coverage.md)、[lab handoff](../../experiments/stack-trace/README.md)。docs-onlyとして独立レビュー済みで、[CI 41461484962](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41461484962)はsuccess。

**捕捉位置は未採択。実Windows traceは未実施。** D01は `blocked/pending` とし、別途承認された使い捨てlabでのtrace、未捕捉mutation評価、独立受入を待つ。これをD04/G1/W01の完了条件の代わりにしない。

## 3. D02の受入証拠

[PR #67](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/67)、merge `23e18b4acb345d04d24ba78e0fe0de15b009866e`。

- 独立Astra reviewは最初にREQUEST CHANGES。二つのfillによる物理slot上書き、判定/寿命の曖昧さ、retire後の保持結果/history予算、failed statusの証明範囲を指摘した。
- rootがwrite admissionごとのvisibility失効・checked incarnation・child provenance、固定decision表、事前予約込みowned-cell上限を修正した。
- 修正後のlocal `8510ba284e5d273e72a71f2d135c23da6c70192c` を独立reviewerが受入、専用checker 5 testを再実行した。
- GitHubに公開した同一source treeのcommitは `f73cc926b2d4b56343eeee1ec096532cc13a590b`、tree `348b18d8b889b9652598f861696c0593eb921a2c`。local/remoteのcommit IDが違うのはGit data APIによるcommit生成であり、tree一致を確認した。
- 受入対象は[coherence ADR](../adr/0013-coherence-model.md)、[trace v1](../contracts/trace-v1.md)、[schema](../../spec/trace-v1.schema.json)、bounded checkerと全24eventを含むfixture。F05 parser、D07 oracle、F06 replayは別作業。

### 実行結果

[CI run 41465272378](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41465272378)の完了と各stepを読み戻した。

| 実行環境 | 証拠 | 結果 |
|---|---|---|
| Ubuntu hosted runner | job 125344366693 | 102 test行がok、全step success |
| Windows hosted runner | job 125344366152 | 100 test行がok、全step success |
| local Linux Rust 1.99.1 | rootの全workspace試験 | 102 passed |
| 独立reviewer Linux | coherence_explorerのみ | 5 passed |

件数はログの実際の `test ... ok` 行から数え、集計行との二重加算や0件test targetを成功件数に入れていない。Windows Serverのhost試験をWindows 11/kernelの実行証拠としない。

実行したコマンド（toolchain/home/target-dirは環境に合わせた一時パス）:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo test -p cache-model --test coherence_explorer --locked --offline
cargo --offline xtask check-planning
```

独立worktreeごとに `CARGO_TARGET_DIR` を分離した。共有targetによるxtaskのcompile-time manifest path再利用を避けるためである。D02単独branchのplanningは37文書/76 link、D01を含むPR merge treeは39文書/78 linkで、いずれも成功。文書追加後の件数変化を失敗や実機証拠と混同しない。

### 探索範囲と証明しないこと

correct 13 case: **2,957 states / 5,697 transitions / 92 terminal states**。14 case/fault組にwitnessを要求した。追加前11 caseは2,919 / 5,661 / 90。追加したStaleOverwrite/SupersededPublishは各19 / 18 / 1。

上限は18steps、2byteセル、2writers、2fills、1slot/hit。二つのfillの追加caseは固定順序であり、任意個数のfillや全Windows concurrencyを網羅するものではない。cutoffはINCONCLUSIVE、正常探索完了と区別する。partial-write effectsは扱うがfailed status自体の完全oracleではなく、D07/F06で別に受入する。普通write completionをdurabilityと同一視しない。

## 4. Public scopeと残るowner/lab gate

2026-11-02の所有者発言は、リポジトリをpublicにしたこと、通常のGitHub Actionsによる積極的な開発検証の許可を明示している。GitHub APIでも `visibility: public` を確認した。所有者の発言をモデルが行った公開操作と取り違えない。

これは以下の承認を意味しない:

- OSSライセンスの採択、外部contribution条件、第三者ソースvendoring（GOV01）。
- 証明書購入、署名identity/鍵、アカウント登録、HLK/製品配布（D08/GOV01/Q04）。
- driver install/load、raw device操作、実機fault injection、VM/lab変更、Secure Boot/HVCI変更（D01/Q02ほか）。
- 人間のストレージ専門家による出荷レビュー。

GOV01/D08/Q02/Q04のowner approvalを推測でapprovedにしない。public visibilityという部分決定はこの文書とIssue本文へ記録し、GOV01全体の `owner_approval: pending` と空evidence配列を維持する。検証済みprivate security reporting窓口も未確認。

## 5. 次の作業

このsnapshotは **done 12 / ready 4 / blocked 35 / needs-decision 1**。readyはD03/D07/D09/F05。D02の採択で解消した依存だけを進める。D01の実trace待ちは、独立したportable作業を止める理由にはしないが、実Windows所有権/捕捉/製品driverのgateを代替しない。
