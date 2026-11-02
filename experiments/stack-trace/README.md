# D01: Codex向け観測実験の引き継ぎ

**実験計画のみ。probe driver未実装・未install・未実行、trace未取得。** G1前の観測実験は製品W01ではない。資料/compile/host model/実Windows traceを混同しない。

## 開始前のstop条件

- 所有者から別途、使い捨てVM/実機、対象fingerprintと許可操作（install、署名mode、mutation、回復）が承認されていること。
- origin/cache/OS diskを識別し、必要なbackup・console・rollbackを確保すること。ここにはformat/install/boot変更の実行scriptを置かない。
- 対象OSは正確なWindows 11 client buildを記録。Server CIをclientの代理にしない。
- 独立reviewされたprobe source/WDK/build hashが必要。capture-only、cache hit/fill/SSD write/準備操作/raw-I/O control endpointなし。

この文書やIssueへの実装依頼、GitHub Actions利用許可だけで上記の承認が得られたとは扱わない。

## 先に準備できるread-only inventory

Codexは承認された機械で以下の既存queryの対応状況を確認できる。機器serial、ユーザーpath、raw memoryは公開しない。非対応flag/APIやpermission errorはUNAVAILABLE、空結果は成功の根拠にしない。

```powershell
pnputil /enum-devices /class DiskDrive /stack /services /drivers
```

Microsoftの[PnPUtil構文](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/pnputil-command-syntax)でbuildごとのflagを確認する。結果にはdevice instance等の識別情報が含まれ得るため、原本はlocalに置き、公開用に匿名化する。追加query候補はIOCTL_STORAGE_QUERY_PROPERTY（StorageAdapterPropertyのSrbType含む）、IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS。これはdevice query実装済みという意味ではない。

**[FSCTL_LOCK_VOLUME](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_lock_volume)はread-only inventoryではない。** cache flushと状態変更を伴う。lock/dismount/offline、BypassIO enable、test-signing/Verifierは別の操作として承認を確認する。

## probeを作る際の限定契約

- upper-disk-class案とlower/SRB案を別artifactにする。class-wide登録を無害と仮定せず、対象外bootへの影響を先にreview。
- forward結果を変えず、boundedな非payload eventだけを記録する。log不足、queue overflow、drop countを隠さない。
- eventには匿名target、request/parent相関ID、major/control/action、range、submit/complete、status/bytesを含める。raw pointerやbuffer内容を出さない。
- unsupported pathは未観測/未対応。非対応hardware上でoperationが失敗しただけで対応済みと判定しない。
- probe自身のcallback・IRQL・buffer lifetime・rundownは、実行前に限定scopeで独立reviewする。製品版の一般契約はD04へ引き継ぐが、D04を待たず未reviewのprobeを実行してよいわけではない。loggingが原因でrequestの寿命を変えない。

## 証拠matrixの埋め方

[coverage](../../docs/contracts/io-coverage.md)の各行ごとに、workload API/handle type、期待される候補経路、実測filter/stack、range translation、forward/completion、欠落counter、rollback結果を記録する。buffered/paging/raw、DSM、zero/offload、ATA/SCSI/protocol、layout/format、BypassIO、lifecycle、no-target/OS diskを省略しない。

各caseの判定はPASS / FAIL / UNSUPPORTED / NOT RUN / UNOBSERVED。UNSUPPORTEDとNOT RUNはPASSではない。coverageの穴は対象縮小・activation拒否・方式変更で処理し、期待値を消さない。

保存metadata: source commit、probe hash、exact SDK/WDK/compiler、OS build、匿名environment ID、geometry/sector pair、stack、許可scopeへの参照、実行case/seed、reproducer、result、未実施、reviewer。実disk内容をpublic artifactに入れない。

## D01の受入

全mutation classを捕捉/拒否できる裏付け、実際のfilter位置、global registrationの影響、drain/再開・復旧の証拠を独立reviewする。それまでD01はpending。D04等のblocked依存を資料だけで解禁しない。
