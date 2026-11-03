# 次のWindows工程: inventory → probe契約 → source/build → 承認済み観測

2026-11-03 JST。D01 #3に戻すための引き継ぎです。**製品が完成して実機調整だけ残った状態ではありません。** portable側の成果と、実Windowsへ接続するための未完部分を分けます。この文書自体は、特定の機械へのアクセスや管理者操作を許可しません。

## 現在あるもの・ないもの

| 成果物 | 現在地 | それだけでは証明しないこと |
|---|---|---|
| [F06 replay](../evidence/F06.md)、[F07](../evidence/F07.md)、[F11](../evidence/F11.md) | 純粋Rustの独立oracle、13固定trace、順序・寿命・error回帰 | 実IRP/MDL/DMA、全mutation捕捉、driverの動作 |
| [P01 audit](../evidence/P01.md) | 収集後のログの欠落・相関・counterを検査するRust CLI | producerの正直さ、取得権限、対象mapping、Windows coverage |
| [D09 proof](../evidence/D09.md) | 小さいRust coreとRust/C-WDK imageのcompile/link・host ABI | driver load、実callback・IRQL・rundown、HVCI適合 |
| [P02 bounded producer](../evidence/P02.md) | Rust core32testsとC scalar56cases、実WDK object-only buildまで受入済み | live callback・transport・rundown・実machine mapping |
| [P03 capture v2](../evidence/P03.md) | PR88で実装・受入済み。Windows296/Linux299のhost CI | producerの診断・実boundaryを自動投影しない |
| [P04観測probe/collector](../contracts/upper-candidate-probe-v1.md) | 限定契約・計画を独立review済み。source/buildは次工程で、現時点では未実装 | D09 entryやP02 snapshotを観測probeと扱わない。P04成功も実機安全性ではない |
| 製品driver・installer・service・Tauri GUI | **未実装**。依存gate待ち | build成功やIssue closeを製品の安全認定にしない |
| 実Windows11 capture・実機性能・電源/撤去試験 | **未実施** | hosted Windows Serverのhost testsでは代替できない |

主実装はRustのまま。Windows境界だけ必要性をADRで説明した小さいC/WDK等を許容します。HDDが唯一の正本、clean read cache / Write-Invalidate、write-backとpersistent cache reuseなし、という方針は変えません。

## 次の小さい受入単位

1. **指定されたWindowsのread-only inventory。** OS/build、実stack、controller/bus、候補targetの関係とqueryの可否を記録する。アクセスできない情報はUNAVAILABLE、空結果はEMPTY。実験成功や対応済みに置換しない。
2. **capture-only probeの限定契約レビュー。** upper候補/lower-SRB候補を別artifactとし、target-specific登録、対象外bootへの影響、forward/completion、context所有権、IRQL、rundown、資源不足、bounded event transport/drop、collector境界と停止方法を具体化する。これは製品位置の採択でもD04一般契約の完了でもない。
3. **レビュー済みprobeのsource/build。** 小さい実装・negative tests・ordinary build CIを行い、source/compiler/WDK/image digestを固定する。P01はJSON投影の契約であってkernel ABI/transportではない。先にABIを勝手に固定しない。
4. **別承認の使い捨てlabで観測。** 対象、install/load、信頼設定、許可workload、rollbackを明示してから実行する。[coverage matrix](../contracts/io-coverage.md)と相関logを照合し、独立review後にD01の採否を判断する。

この順序は、すべてのsource/build準備がinventory完了を待つという意味ではありません。[P02 #83](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/83)はbounded producerと非loadable snapshot objectまで受入済み。[P04 #89](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/89)では、未確認の対象機械を仮定せずcallback・forward・rundown・control transportを扱う限定契約を独立reviewしました。次は同じdriver sourceの疑似DDI試験と実WDK link、Rust collectorのsource/buildです。source準備と別承認の実機観測を区別します。

source/build準備をG1前に行うこと自体は禁止していません。ただし、実stackの未確認事項を都合のよい仮定で埋め、既存のD01/D04/D10/F08/F09/G1を解除しません。新しい小作業は依存・scope・受入条件を独立reviewしたうえでIssue化します。単にready件数を増やすためにgateを消しません。

## 最初に指定が必要な情報

| 項目 | 必要な指定・確認 |
|---|---|
| inventoryの対象 | どのWindows11 VM/PCでread-only調査を行ってよいか。実験用targetへのアクセス許可とは別 |
| exact environment | Windows build/revision、architecture、候補bus/controller/driver stack。現在値は実際に確認し、過去の会話から確定しない |
| 将来のlab候補 | 廃棄可能なデータ用target、OS/boot/pagefileとの分離、snapshot/backup/console/rollbackの可否。disk番号だけで恒久identityとしない |
| 変更操作の権限 | install/load、mutation、署名mode、Verifier、power/removeは操作別に明示。read-only inventory許可から推論しない |
| 所有者判断 | GOV01のlicense/外部貢献/製品配布、D08の署名主体・鍵・費用等は別。public化とActions許可だけでは完了しない |

最初の工程にformat、raw write、lock/dismount/offline、boot/security設定変更、証明書購入は不要です。VMware/Hyper-V/App Sandbox等の過去の検討だけで現在のlab構成を推測しません。

## Codexへ渡す最初のプロンプト

以下は**ユーザーがinventory対象として指定したWindows環境**で使う文面です。指定がなければ対象だけ確認して止めます。driverをinstallするプロンプトではありません。

```text
H4M4CHi-ttr/win-ssd-hdd-cache の D01 #3 のため、指定されたWindows環境のread-only inventoryを行ってください。
まず最新main、AGENTS.md、docs/handoff/2026-11-03-windows-next-step.md、experiments/stack-trace/README.md、ADR-004、io-coverage.md、D09/P01の証拠と未完gateを確認してください。既存のdirty worktreeを保存してください。

目的は実stackの事実と不明点を返すことだけです。観測probe/製品driverはまだありません。D09 imageをloadせず、P01のinternally_completeをD01 PASSにしないでください。

1. 対象Windowsの正確なbuild/revision・architecture、Git/Rust/VS/SDK/WDKの導入状態を、既存のread-only queryで確認する。新たなインストールはしない。
2. Microsoftの当該build向けPnPUtil構文と pnputil /? を確認し、許可された権限の範囲でdisk deviceのstack/services/driversを照会する。必要なら以下のenum queryだけを使う:
   pnputil /enum-devices /class DiskDrive /stack /services /drivers
   権限不足、非対応flag、失敗、空結果は区別して報告。自動昇格や設定変更で回避せず、必要な限定queryと理由を提示して確認を求める。
3. origin候補、cache候補、OS/boot/pagefileを取り違えないための既存情報を整理する。serial/instance ID/ユーザーpath等を含む原本はlocal/privateに保持する。公開文書にはfreshな匿名ラベルと必要な非機密要約だけを使い、raw出力をIssue/Actionsへuploadしない。
4. 不明なrange変換、SRB形式、filter順序、target専有性を推測しない。query実装がまだないなら「未実装/未確認」とする。PnP inventoryは実mutation coverageの証拠ではない。
5. 得られた事実、実行command/exit、未確認事項、probe限定契約で先に解決すべき問いを返す。この工程でsource/driver/registry/storage/boot設定を変更しない。

禁止: driver install/load、format/raw write、lock/dismount/offline、BypassIO変更、Secure Boot/HVCI無効化、test-signing/Verifier変更、障害注入、購入・release。これらは別の明示承認とreview済みlab手順が必要です。
```

inventoryが戻れば、Pro/独立review側でprobeの限定契約とsource/build準備を続けられます。必ずしも同じ製品名のエージェントである必要はありませんが、指定Windows環境にアクセスできる実行者が必要です。

## 一次資料で再確認した注意点

2026-11-03にMicrosoft資料を再確認しました。

- [PnPUtil構文](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/pnputil-command-syntax): `enum-devices`の`stack/services`と`drivers`には導入versionの違いがあります。文書は管理者command promptを案内しており、すべての環境で非昇格の成功を保証しません。権限不足は情報不足として扱います。
- [storage filter dispatch](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/storage-filter-driver-s-dispatch-routines): class/port間ではSRB形式の識別が必要です。文書のsampleは対象構成の全経路・寿命・回復の証拠ではありません。
- [FSCTL_LOCK_VOLUME](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_lock_volume): cached dataのflushとvolume状態の変更を伴います。read-only inventoryに混ぜません。

既知のD09 runner-image表示差は原因未解決のまま保持します。署名/HVCI・人間の専門家レビュー・実機qualificationも未完のままです。これらをmodel件数やドキュメント量で代替しません。
