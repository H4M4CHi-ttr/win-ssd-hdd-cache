# 判断記録（ADR index）

「採択」は設計方針の採択であり、実装や安全性試験の合格ではありません。

| ADR | 状態 | 判断 | 理由 / 再検討条件 |
|---|---|---|---|
| ADR-001 | 採択 | clean read cache / Write-Invalidate、HDD唯一原本 | write-back固有リスクを初期scopeから外す。read cache自体の整合性リスクは残る |
| ADR-002 | 採択 | cold reset、初期persistent cacheなし | offline mutation/reboot freshnessの難題を初期版から外す |
| ADR-003 | 採択 | 専用data HDD + 別SSD専有partition、明示opt-in | 再帰I/O、原本format、対象誤認を避ける |
| [ADR-004](adr/0004-interception-boundary.md) | D01資料統合済み・受入未完 | storage/block filter候補、位置は未採択 | raw/control pathと対象外bootへの影響は別承認labのtrace待ち |
| [ADR-005](adr/0005-cache-engine.md) | D03 host方針受入・main統合済み | original Rustを後続host qualificationへ採択、今回のOCF移植は見送り | OCF実行比較・F09・製品engine受入は未実施。Windows境界とは別 |
| ADR-006 | 採択 | 主実装Rust、必要なGUIはTauri | 所有者の明示要件。kernelだけ必要に応じC/WDK等を局所採用 |
| [ADR-007](adr/0007-rust-kernel-boundary.md) | D09 build-only受入・main統合済み | Rust-firstと最小C/WDK fallbackのビルド・ABI比較 | production entry/glueは未選定。IRQL/MDL/DMA/HVCI/HLK・driver loadの証拠ではない |
| ADR-008 | 採択 | CLI/serviceを先に、非特権Tauri UIは後段 | UIとkernelの危険な依存を切り、安全条件を再利用 |
| ADR-009 | 採択 | unsafe境界・ABI・同時実行は上位担当/独立review | Lunaに未定義の安全契約を決めさせない |
| ADR-010 | 設計ゲート D08 | 一般公開はWHCP/HLK条件確認を基本 | attestation/test trustと製品公開条件を混同しない |
| ADR-011 | owner decision GOV01 | original code MIT OR Apache-2.0を提案 | source repositoryは所有者判断でpublic。license/外部貢献/vendoring/製品配布は未決 |
| ADR-012 | 採択 | 原本の暗黙変更、危険な実機自動試験は禁止 | task実装許可とdisk消去/OS安全機能変更の許可は別 |
| [ADR-013](adr/0013-coherence-model.md) | D02受入・main統合済み | token/writer数/physical lease/content provenance | bounded checkerと二OS host試験。実Windows安全性・一般的証明ではない |
| [ADR-018](adr/0018-capture-probe-language-boundary.md) | P04 source/build受入・main統合済み | 実験用observerの限定lifecycleだけC17/WDK、collector/archive/auditはRust | 製品driverの言語や捕捉位置は未採択。storage payload/MDL/DMA/cache I/Oへの拡張は別review |

B03のresource-policyは将来のADR-019として予約します。採択済みADR-018との番号衝突を直しただけで、B03を受入・readyにしたものではありません。

## 個別ADRの完了形式

D系IssueのPRは `docs/adr/NNNN-short-name.md`（planned）を追加し、Context / Options / Decision / Evidence / Consequences / Rejected alternatives / Revisit trigger / Reviewer / Related issue を記載する。選択肢の比較に使ったコードはlab-onlyであることを表示する。

「動いた」「Rustなので安全」「OCFに任せた」だけをEvidenceにしない。未検証の条件は明記し、後続Issueの契約/依存/ラベルを同じPRで更新する。
