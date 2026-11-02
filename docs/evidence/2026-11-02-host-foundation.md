# Issue監査後のhost基盤実装・検証 — 2026-11-02

## 結果と範囲

元の作業49件と親Issueを全件監査し、[設計/依存の是正](../issue-audit-2026-11-02.md)を行った。追加したAUD01/D10/W10を含め、現在は作業52件＋親1件。root/Proが実装したhost課題は **F01/F02/F03/F04/C01/C02/C03/B01/B02/F10の10件**。それぞれ作者とは別の指定強度のレビューを受けた。AUD01の最終文書・Issue同期はこの記録とbacklogで追跡する。

**mainへのmergeは未実施、作業Issueもcloseしていない。** 成果物の受入とmainへの統合は別。PR上の受入済み成果物を後続で使う場合、そのbranch/commitを基底にする。公開設定・所有者承認・ライセンスは変更していない。

実装はsafe Rustの範囲計算、決定的fake-I/O queue、unknown discardを持つin-memory origin、strict status/benchmark codec、診断allowlist、offline CLI、計画checker、host CI。**HDDを高速化できる完成キャッシュ、Windows driver、Windows 11/実機の安全性証明ではない。** Tauri GUI・WDK/FFI・実デバイス操作は未着手。

## PRとレビュー

各PRは一つのIssue受入単位に対応する。PRは表順の積み上げであり、code依存のない課題を不必要に意味上依存させたわけではない。shared Cargo manifestの統合を直列にした。

| 成果物 | 作業Issue | PR | 独立レビュー | remote head |
|---|---|---|---|---|
| F01 | [#5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) | [#54](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/54) | SOL | `55bb4efe8075` |
| F03 | [#7](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/7) | [#55](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/55) | SOL | `70880a059322` |
| AUD01 | [#51](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/51) | [#56](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/56) | Astra | `fbaa5cf3a4a8` |
| F04 | [#12](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/12) | [#57](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/57) | SOL | `a741b2a5fd49` |
| C01 | [#16](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/16) | [#58](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/58) | SOL | `3d4e603a9c70` |
| B01 | [#18](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/18) | [#59](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/59) | SOL | `adc9568172c4` |
| C03 | [#21](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/21) | [#60](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/60) | Astra | `0bb380c63768` |
| C02 | [#22](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/22) | [#61](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/61) | SOL | `f3f60cb72f2d` |
| F02 | [#10](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/10) | [#62](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/62) | SOL | `5e7b0850b386` |
| B02 | [#28](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/28) | [#63](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/63) | SOL | `8eb54ef7620b` |
| F10 | [#13](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/13) | [#64](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/64) | SOL | `f5c73ebcd13b` |

GitHubのReviewイベントによる人間の承認と、上表のAI独立レビューは区別する。人間のWindowsストレージ専門レビューが必要な後段ゲートをAIが代替したわけではない。後続のmetadata-only PRはAUD01に属する。

## 実行した検証

[Actions run 41416024821](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41416024821)、PR #64 head `f5c73ebcd13b756bbf7d235aff0986f948dd9438` を読み戻した。Rust 1.99.1、固定Cargo.lock、checkout v7.0.1をfull SHA pin。

| 実行環境 | target | 実際に合格したtest | fmt / clippy / planning |
|---|---|---:|---|
| ローカルLinux・独立reviewのfresh archive | x86_64-unknown-linux-gnu | 97 | PASS |
| GitHub-hosted Ubuntu 24.04 | x86_64-unknown-linux-gnu | 97 | PASS |
| GitHub-hosted Windows Server 2025 | x86_64-pc-windows-msvc | 95 | PASS |

どのjobにもignored/failedはない。OS固有テストの差（Unix-only 3件、Windows-only 1件）で件数が2件異なる。ログ末尾でguardが再表示する合格行は二重計上していない。全jobのstep結果と生logのtest一覧/実行summaryを照合した。Windows Serverのuser-mode試験はWindows 11 driver qualificationではない。

実行command:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo xtask check-planning
```

ホスト試験はin-memory /普通のtemporary filesと固定test shellのみ。実disk、driver install、WDK/Verifier、電源断、署名鍵は使っていない。ローカルで依存取得後の実行は `--offline` も付けた。[開発手順](../development.md)を参照。

### 実際に発見して直した不具合

1. **C01: 巨大なJSON数値tokenの分類。** `1e400`等の有効JSONをsyntax errorと扱っていた。独立SOL指摘→RED regression→raw数値tokenを保つvisitor修正→再レビュー。値を成功扱いしたのではなくschema errorへ正しく拒否した。
2. **F10: ignored-onlyの偽成功。** discoveryだけでは全件ignoredでもPASSになり得る。独立reviewの実Rust再現→RED2件→実際のpassed数を確認、pipefailでCargo failureを保持→再レビュー。ゼロ/ignored/正例/失敗出力のbehavioral testを追加。
3. **F10: WindowsのBash選択。** [初回run 41415585065](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41415585065)はUbuntu成功、Windows失敗。native processがWSL launcherを選択していた。Git Bashの明示path、portable選択regression、shell正例controlを追加し、再runで両OS成功。WSL導入、テストskip、期待値の緩和はしていない。

## 再現性と書き込み範囲

local環境はremote Git履歴を持たないAPI snapshotだったため、GitHub Git-data APIで既存remote parent/treeにIssue単位のcommitを作成した。force pushは使っていない。変更ファイルをblob SHAでlocal reviewed sourceと照合し、未変更ファイルはremote baseのblobを維持した。初期local snapshotの22文書にはそれぞれ余分な末尾LFが一つあったことを全件比較で確認し、無関係な10ファイルはremoteの元byteを保持した。

[依存inventory](../dependencies.md)は14 registry archivesとCargo.lock checksumを照合済み。これは完全な法務/脆弱性監査ではなく、project license採択でもない。host workflowはconfigured secretsなし、read-only job token、credential非保持、GitHub-hosted runnerのみ。静的recipe検査は任意Rust/dependency/build scriptの安全sandboxではない。

## 残る重要な境界

- D01: Windowsの全mutation捕捉/拒否経路の実証。
- D02: 整合性・read/write lease・世代・slot再利用・linearizationの契約凍結。
- D09/D04/D10: Rust/WDK適合性、IRP/MDL/DMA寿命とportable oracleとの対応。
- G01はB03の予算とD10を含む前提を維持。G2以降、実機/HVCI/HLK、人間の専門レビューは未実施。
- GOV01/D08/Q02/Q04のowner approvalはすべてpending。URL、AIレビュー、CI成功から承認を推測しない。

個別evidence文書のtest件数やNOT RUNはその実装時点の履歴。上記のrunは今回の統合host基盤の結果であり、それ以前の失敗をなかったことにしない。最終的なbacklog/Issue readbackはAUD01のcloseoutレビューで確認する。

