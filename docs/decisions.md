# 判断記録（ADR index）

「採択」は設計方針の採択であり、実装や安全性試験の合格ではありません。

| ADR | 状態 | 判断 | 理由 / 再検討条件 |
|---|---|---|---|
| ADR-001 | 採択 | clean read cache / Write-Invalidate、HDD唯一原本 | write-back固有リスクを初期scopeから外す。read cache自体の整合性リスクは残る |
| ADR-002 | 採択 | cold reset、初期persistent cacheなし | offline mutation/reboot freshnessの難題を初期版から外す |
| ADR-003 | 採択 | 専用data HDD + 別SSD専有partition、明示opt-in | 再帰I/O、原本format、対象誤認を避ける |
| ADR-004 | 設計ゲート D01 | storage/block filterを第一候補、位置は未決 | raw/control pathと対象外bootへの影響をtraceで確認 |
| ADR-005 | 設計ゲート D03 | OCF WI vs 最小clean-cache core | porting負債、C/Rust境界、memory、整合性の実証で選択 |
| ADR-006 | 採択 | 主実装Rust、必要なGUIはTauri | 所有者の明示要件。kernelだけ必要に応じC/WDK等を局所採用 |
| ADR-007 | 設計ゲート D09 | kernel Rust / no_std / WDK shimの責任分割 | runtime/panic/alloc/IRQL、toolchain/HLK、保守性を実証。Rust採用を安全証明としない |
| ADR-008 | 採択 | CLI/serviceを先に、非特権Tauri UIは後段 | UIとkernelの危険な依存を切り、安全条件を再利用 |
| ADR-009 | 採択 | unsafe境界・ABI・同時実行は上位担当/独立review | Lunaに未定義の安全契約を決めさせない |
| ADR-010 | 設計ゲート D08 | 一般公開はWHCP/HLK条件確認を基本 | attestation/test trustと製品公開条件を混同しない |
| ADR-011 | owner decision GOV01 | original code MIT OR Apache-2.0を提案 | 未採択。LICENSE/公開/第三者取り込み前に決定 |
| ADR-012 | 採択 | 原本の暗黙変更、危険な実機自動試験は禁止 | task実装許可とdisk消去/OS安全機能変更の許可は別 |

## 個別ADRの完了形式

D系IssueのPRは `docs/adr/NNNN-short-name.md`（planned）を追加し、Context / Options / Decision / Evidence / Consequences / Rejected alternatives / Revisit trigger / Reviewer / Related issue を記載する。選択肢の比較に使ったコードはlab-onlyであることを表示する。

「動いた」「Rustなので安全」「OCFに任せた」だけをEvidenceにしない。未検証の条件は明記し、後続Issueの契約/依存/ラベルを同じPRで更新する。
