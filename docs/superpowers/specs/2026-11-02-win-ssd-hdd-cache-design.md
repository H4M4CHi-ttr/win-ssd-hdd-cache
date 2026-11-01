# Windows SSD/HDD clean read cache — 正式設計 v0.1

作成: 2026-11-02 JST  
状態: **設計ベースライン。製品実装なし、Windows/実機の安全性は未検証。**  
責任範囲: 本リポジトリの設計・検証・実装・配布。破損を許容できない実データを使う実験は行わない。

## 1. 目的と成功条件

既存NTFSのデータ用HDDを再フォーマットせず、安価な別SSDをキャッシュに利用し、繰り返し発生するランダム読み取りを短縮する。開発ソース、依存パッケージ、インデックス作成等を想定するが、初回読み取りと書き込み中心の処理がSSD化されるとは約束しない。

キャッシュはバックアップではない。SSDに唯一の未反映データを置かないことは、write-back特有の損失範囲を減らすだけである。古いNTFSメタデータを返せば、その後のOSの書き込みによりHDDの原本も破損し得る。

成功条件は、(a) 原本のI/O意味論を弱めない、(b) キャッシュ障害・無効化・撤去の検証、(c) Windows RAMキャッシュとの比較で実作業上の利益がある、(d) 普通の利用者にSecure Boot/HVCIの無効化を要求しない配布、の全て。

## 2. 初期サポート境界

以下は実証対象の**候補条件**であり、現時点のサポート表ではない。特定Windows build・WDK・接続方式・機器・ドライバースタックを記録した資格試験が完了するまで、supportedとは表示しない。

| 項目 | 初期候補 | 初期対象外 / 拒否 |
|---|---|---|
| OS | Windows 11 x64、実証したbuildのみ | ARM64、Windows Server、未試験build |
| 原本 | 内蔵の専用basic HDD、単一NTFSデータボリューム、固定レイアウト | OS/boot/EFI/recovery、pagefile、crashdump |
| キャッシュ | 別物理SSD上の専有・未マウント専用partition | 原本と同一disk、NTFS上のキャッシュfile、外部から同時使用 |
| 接続 | 内蔵SATA HDD + 内蔵NVMe SSDを最初の試験候補 | USB、hot-plug、RAID、Storage Spaces、dynamic disk |
| FS/暗号化 | 非暗号化NTFS、plain basic disk | BitLocker/device encryption、ReFS/Dev Drive、暗号化境界が未確認の構成 |
| 更新・電源 | attach/restart/resume時にcold reset | キャッシュ永続再利用、multi-boot中の継続利用 |
| 書き込み | HDDへ通常の要求として渡し該当キャッシュをinvalidate | write-back、SSD上だけでのwrite完了 |
| 管理 | 管理者の明示的opt-in、対象identity一致 | drive letterだけの選択、自動変換、自動format |

暗号化境界は利用者の自己申告だけで保証しない。BitLocker等の対象検出とfilter位置はD01/D05/D06で判定し、不明なら有効化しない。EFS等のfile-level暗号化は、全ファイル列挙を前提にせず、FS下層の配置で見えるbytesと必要な拒否条件を実証する。初期版でもcacheの残留データの扱いを説明する。

**ページファイルを対象外にしても、memory-mapped file等のpaging I/Oは無視できない。** OSの通常アクセス経路として試験する。

## 3. 選択肢と決定方針

| 方式 | 長所 | 制約 | 判断 |
|---|---|---|---|
| Windows storage/block filter | 既存NTFSを透過的に使う目的に近い | IRP、raw I/O、寿命、PnP/power、署名の難度 | 第一候補。捕捉位置はD01の証拠で決める |
| WinFsp user filesystem | userspaceで試作しやすい | 別namespace、迂回アクセス、NTFS機能差 | 製品の本線にしない |
| WinSpd等の仮想disk | userspace engine実験が可能 | 原本の排他的所有・二重mount防止・配布状態 | 必要時の実験候補。採用を前提にしない |
| Linux bcache/dm-cache移植 | 実績のある概念とコード | Linux block層への強い依存、既存NTFS保持 | 丸ごと移植を初期戦略にしない |
| OpenZFS/L2ARC | 既存のread-cache設計 | ZFSプールへの移行、Windows版の成熟度 | drop-in NTFS cacheという本目的とは別 |

エンジンは **OCF Write-Invalidate (WI)を候補**とする。OCFはWindows用の完成ライブラリではない。環境ラッパー、I/O所有権、メモリ、同期、sector条件、license通知の実証が必要。Write-Around (WA)とは異なり、WIではwrite hitもinvalidateしてHDDへ渡す。

D03でupstream commitをpinし、最小独自clean-cache coreとの比較を行う。採否基準は安全契約の実現性、テスト再利用、porting量、nonpaged memoryと保守性。採択前に「OCF統合実装済み」やWindows互換を主張しない。上流コードの複製はlicense採択・通知手順後。

## 4. 構成と責務

1. **kernel data path**: 通常I/Oの転送、整合性境界、cache hit検証、原本/SSD I/O寿命、PnP/power。管理サービスへの同期依存を持たない。
2. **portable engine adapter**: 選定エンジンの最小インターフェース。Windows IRP/MDLを内部policyに漏らさない。policyとI/O所有権を混同しない。
3. **control service + CLI**: Rustで対象の検査、状態表示、明示的enable/disable、診断を実装する。Windows APIは公式のwindows系crate等の必要最小限のbindingで包む。GUIが必要になった段階でTauri + Vue/TypeScriptを追加し、同じversioned IPCを利用する。
4. **offline model + test harness**: Rust workspaceの独立crate。実diskを開かず、決定的なイベント列とbyte oracleで故障を再現する。製品engineとoracleの実装を共有し過ぎず、同じバグで同時に合格する循環検証を避ける。
5. **release/qualification**: VM・実機の証拠、署名、互換性allowlist、回復手順。

**主要実装はRust、GUIはTauri**を採択する（所有者の明示要件）。型・モデル・CLI・管理サービスはRust。Windows kernelの部分だけはD09/D04でRust no_stdの実用性、WDK互換性、panic/alloc/IRQL、署名/検証ツールを実証し、必要ならC17 + WDKの薄いshimを使う。OCFを採る場合もそのC実装を全面移植せず、局所的FFIで隔離する。言語例外には範囲・理由・ownership契約をADRに残す。Microsoftのwindows-drivers-rsは現在の公式READMEの成熟度注意を確認し、名前だけで本番利用可能と判断しない。Rustの型安全性で同期・DMA・電源断の正しさを代替できるとは扱わない。GUIは後段であり、最初の安全性ゲートに含めない。

管理面とデータ面の境界は、検証済み・バージョン化したcontrol ABIのみ。状態表示のJSONとkernel IOCTLのbinary layoutは別契約である。Rust structのmemory layoutやenum discriminantをそのままwire/FFI契約とみなさない。unsafeは境界moduleへ隔離し、SAFETYコメントと独立レビューを必須とする。Tauri/WebViewを管理者権限で常用せず、privileged service側で認可・再検証する。

## 5. 安全契約（全て必須）

### INV-01 原本・完了・durability

HDDが唯一の原本。writeの成功/失敗・bytes completedを下位経路より楽観的に変更しない。成功ACKを早めない。HDD write completionと電源断後の永続化は同義ではない。FUA、write-through、flushを正しく伝え、filter内に保留された先行writeをflushが追い越さない。原本のerrorを古いcacheやゼロで隠さない。

### INV-02 mutation coverage

通常writeに加え、paging/mapped I/O、DSM/TRIM/UNMAP、zero/offload/copy、raw physical I/O、pass-through、resize/reformat、snapshot restoreを分類する。捕捉できない経路が残る構成ではcache hitを許可しない。未知のpotential mutationを、cacheを有効のまま単純passthroughしない。quiesce/invalidate後のbypass、明示的拒否、または構成全体の非対応をD01で定義する。

### INV-03 freshness / overlap

invalidate-before-writeだけでは不十分。read(old)→write(new)→late-fill(old)を禁止する。各cache rangeの世代、active writer数、read/fill token、session epoch、slot incarnationを用いた公開条件または同等以上のプロトコルをD02で固定する。

readとwriteが重なった場合の元々のWindows意味論以上に、multi-sector atomicityを約束しない。一方、完了したwriteの後に新たに開始したreadへ古いcacheを返してはならない。partial writeは交差する全cache lineを無効化する。write失敗/部分完了後も該当rangeはinvalidのまま。複数writerの一方が完了しただけでpendingを解除しない。

### INV-04 fill / slot 再利用

fillの公開は、観測した世代、writer不在、session、slot所有権を再検査してから行う。旧セッションのSSD writeが新slotを後から物理的に上書きし得る。**epochの更新はI/O停止の代わりにならない。** 旧I/Oの終了・所有権回収を確認するまで同じSSD slot/領域を再利用しない。確認不能ならcacheを再開せずbypassに固定する。

### INV-05 buffer / cancellation

cancel requestはcompletionではない。SSD timeout後にHDDへfallbackした場合でも、旧SSD I/Oがcallerのbuffer/MDLへ遅延DMAできてはならない。private owned buffer、child IRP、MDL、caller request、target object、slotの参照・解放責任をD04で固定する。foreground完了後にcaller bufferをbackground fillで参照しない。

### INV-06 fault containment

検出可能なcache checksum mismatch/read/write errorは該当entryをinvalidateし、必要ならHDDへ再読する。checksumはbitrot検出でありfreshness証明ではない。原本errorはそのまま伝える。SSD hang時のcancel/drainが有限時間に完了するという保証はしない。resourceを上限管理し、新しいcache workを止める。故障したcacheから返したデータを成功扱いしない。

### INV-07 identity / exclusive ownership

drive letter/disk numberではなくdevice ID、partition GUID、開始offset、length、sector geometry、sessionを検証する。disk複製、identity再利用、resize、再format、対象交換、同一disk/範囲/依存経路重複を試験する。同一identityであることは内容のfreshnessを証明しない。cache partitionは専有し、自己再帰するI/O経路を作らない。

### INV-08 lifecycle / reboot

attach、reboot、resume、hibernate復帰、offline/online、対象再認識はcold cache。clean-shutdown flagがあっても前回cacheを再利用しない。raw SSD上の旧データ/metadataが残っていてもvalidと認めない。解除時はnew work停止→hit停止→epoch切替→rundown→必要時reboot。データ用disk限定でも、global filter登録がOS diskやbootに影響しない保証にはならない。D01/D05で実stackを検証する。

### INV-09 bounded kernel work

spinlockを保持した下位I/O待機は禁止。IRQL、lock ordering、nonpaged allocation、queue上限、最大inflight、1要求のrange上限、low-memory時のbypass方針をD04で数値化する。work queueを無制限に増やさない。管理サービス停止時もHDDへの通常経路を止めない。

### INV-10 security boundary

control操作は管理権限が必要。read-only statsとmutation操作を分ける。ABIのversion/size/enum/range/整数overflow/ACLを検証し、任意kernel pointer、任意physical I/O、任意code実行を公開しない。NTFSのACLを迂回して他人のcache bytesを読める管理機能を作らない。キャッシュは削除済みファイルの内容も残し得る。BitLocker等の境界を無意識に跨がない。

### INV-11 recoverability

無効化、service failure、cache喪失、upgrade/rollback、uninstallの経路を製品機能として試験する。原本HDDをformat/partition変更しない。cache専用partitionの初期化はdry-runと対象fingerprint確認後、別の明示承認を必要とする。全disk消去コマンドを既定手順にしない。キャッシュが壊れたからといって自動的に任意SSDを初期化しない。

### INV-12 evidence and honesty

model、VM、実機、Driver Verifier、HVCI、HLK、署名を別の証拠として扱う。AIレビューは人間のWindowsストレージ専門家の出荷前レビューを置き換えない。未測定性能・未対応build・破損ゼロを宣伝しない。Issueのcloseや署名だけを安全性証明としない。

## 6. データパスの規定

### Read

RAM cacheを通過して到達したI/Oのみが本driverの観測対象。対象rangeにactive mutationがある、状態がACTIVEでない、entryがない/不正/不一致ならHDDへ渡す。cache hit候補はepoch/range/slotを検証し、owned buffer経由で安全に返す。失敗時はHDDへfallbackする。miss結果のSSDへの取り込みはbest-effortであり、原本readの成功を取り込み失敗により失敗へ変えない。

### Write / mutation

可視化前に交差rangeをinvalidateし、後続read/fillとの順序を保つ。HDDへの要求、completion、barrierをpreserveする。payloadはwrite-back用にSSDへ保持しない。background fillとの競合条件はD02/D04に従う。discard後の内容は未定義であるため、test oracleはゼロと決めつけない。

### Flush

原本へのflush順序と完了結果をpreserveする。clean cacheのSSD fillを永続化するためだけに、HDDのflushを不必要に待たせない。ただし既に受け取ったorigin writeの順序を壊さない。この分離をD04のABIとQ系試験で検証する。

### Cache reset

validityを破棄する操作と、旧I/Oが物理的に終了する操作を区別する。停止・quiesceが確認できない領域は再利用禁止。再起動後は旧kernel I/Oは存在しないが、device状態を再確認し、前回metadataを採用しない。

## 7. 状態機械（概念契約）

| 状態 | cache hit | 新規fill | HDD経路 | 遷移条件 |
|---|---:|---:|---|---|
| DISABLED | なし | なし | 通常転送 | 管理者opt-in + preflight成功 |
| VALIDATING | なし | なし | 通常転送 | identity/排他/互換性確認 |
| SHADOW | なし | 実験用のみ | 全readはHDDから返す | 安全/性能観測、同世代のみ比較 |
| ACTIVE | 条件付き | 上限内で許可 | miss/write/flushを転送 | 全ゲートと管理者opt-in |
| QUIESCING | 新規なし | 新規なし | 安全に転送 | 所有権/rundown確認待ち |
| DEGRADED | なし | なし | 可能な場合passthrough | cache fault、quiesce不能 |
| REMOVING | なし | なし | PnP/stack契約に従う | 参照回収後のみ解放 |

D05が実際のPnP/power状態との対応とlock順序を凍結する。DEGRADEDからの自動再開は禁止。再検証と旧I/O終了を確認した明示操作が必要。raw passthrough等で安全を維持できない場合、HDDへの要求を勝手に成功させず、規定の拒否/失敗を返す。

## 8. 資源・性能・policy

128〜256 GB級SSDを想定しても全容量を無条件にindex化しない。32 bytes/entryという仮の例では、128 GiB cacheのindexは4 KiB lineで1 GiB、64 KiB lineで64 MiBになる。これはOCFの実測値ではなく、block sizeとmemory budgetを測る理由の例。

D03/B03でline size、max cache size、nonpaged budget、promotion、scan cutoff、write amplification、queue depthを決める。最初はpolicy調整より正しさを優先し、cache pressureはmiss扱いで逃げる。全disk scanでhot dataを追い出す問題と、cache fillがHDDを競合させる問題を測る。

64 GB級RAMのPCでは、普段のhot setがWindows RAM cacheに収まる可能性がある。SSD hit率だけで価値を主張せず、cold/warm OS cache、cold/warm SSD cache、作業時間、p95/p99、CPU、nonpaged memory、SSD書込量を比較する。

## 9. 段階と出荷ゲート

- G0: 設計判断・法務/配布の見通し。ライセンスと公開はowner決定。
- G1: 決定的offline model、反例検出、engine適合性、捕捉/所有権/制御ABIの凍結。
- G2: 使い捨て環境でpassthrough-only → mutation観測 → SHADOW。HDD内容はcacheから返さない。
- G3: 明示opt-inしたlabだけでcache hit提供。fault、resource、PnP/power、取消し、回復を実証。
- G4: 人間の専門レビュー、指定build/機器の実機試験、署名/HLK/配布・更新・撤去証拠をそろえて限定beta判断。
- G5（将来別RFC）: persistent read cache、暗号化、複数SSD、GUI、広い互換性。write-backは本ロードマップの対象外。

G1/G2/G3/G4の具体的acceptanceは [検証文書](../../validation.md)。各ゲートは独立レビューで閉じる。未解決の前提をLunaの実装Issue内で暗黙に確定しない。

## 10. 未決事項と所有者

| 未決事項 | 所有Issue | 完了時の成果物 |
|---|---|---|
| filter位置と全mutation path | D01 | 実stack/trace/拒否表 + ADR |
| 同時実行/linearization/世代 | D02 | state model/反例/公開条件 |
| OCF採否・pin・memory | D03 | 適合性試験 + ADR + wrapper一覧 |
| kernel Rust / WDK境界 | D09 | toolchain実証 + ADR + no_std/panic/allocator条件 |
| async ownership/timeout/IRQL | D04 | ownership表/ABI/数値上限 |
| 対象判定/power/撤去 | D05 | identity schema/状態遷移/回復手順 |
| privileged control ABI | D06 | IOCTL/ACL/IPC脅威分析 |
| 故障oracleと耐久性 | D07 | 注入点/expected outcomes/証拠schema |
| 署名・HLK・配布主体 | D08 | 条件/費用区分/公開前checklist |
| OSSライセンス/公開可否 | GOV01 | owner decision/通知・貢献条件 |
| 実用policy・性能目標 | B03 | 実測と予算/採否基準 |

安全に関わる未決事項は、明記されたゲートとして後続作業を止める。見積りのための仮定は、実装契約として使わない。
