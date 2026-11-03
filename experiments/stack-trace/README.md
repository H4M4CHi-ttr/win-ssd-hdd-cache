# D01: Codex向け観測実験の引き継ぎ

**runnable probe driver・collectorは未実装、未install・未実行、trace未取得。** このdirectoryには実験計画とP02の限定source/build準備を置きます。G1前の観測実験は製品W01ではない。資料/compile/host model/実Windows traceを混同しない。

2026-11-03追記: [P01のoffline capture audit](../../docs/evidence/P01.md)はPR #81で実装・受入済みですが、producer/collectorを実装したわけではありません。[次のWindows工程とCodexプロンプト](../../docs/handoff/2026-11-03-windows-next-step.md)はread-only inventoryから始まります。

## P02: source/buildだけを準備する部分

[P02契約](../../docs/contracts/capture-producer-v1.md)に従う小さいscalar snapshotです。製品の捕捉位置は未採択であり、実際のdispatch/completion callbackへ接続していません。

| ファイル | 限定した責務 |
|---|---|
| `include/p02_snapshot.h` | Rustと同じ整数layout、strict validation、host scalar constructor。submit48/8・completion24/8を自然配置で検査 |
| `host/snapshot_vectors.c` | 手導出20ベクトルと36 strict case。実IRPや偽WDK構造体を動かす試験ではない |
| `wdk/upper_snapshot.c` | WDKの公開型から許可されたスカラーだけをコピーする普通の関数。entry/callback/forward/register/attachなし |
| `tools/check-object.mjs` / `tools/test-object.mjs` | bounded AMD64 COFFのsymbol構造チェック。memory access・relocation・runtime安全性の証明ではない |
| `tools/build.ps1` | ephemeral Windows Actionsでhost試験とWDK object-onlyコンパイル。kernel imageをlinkせず、LLVMもdriverもinstallしない |
| `tools/header-paths.ps1` / `tools/test-header-paths.ps1` | compilerの絶対working directoryを明示したincludeパス正規化と17件のWindows文字列試験。許可root外・prefixが似ただけの別directoryを拒否 |

Windows側はown live/stable stackからforward前にsubmitを取り、completionは同じ要求のimmutable saved classificationを使う想定ですが、**その寿命・同期・IRQL・呼出し経路は未実装の統合前提**です。入力/outputのnonalias、resident memory、IRQL<=DISPATCH_LEVELを呼出し側が証明する必要があります。null checkは任意pointerを検証しません。戻り値0/1/2はsnapshotの結果でありNTSTATUSではありません。

Read/Writeのminor0だけを普通のtransferとみなし、負のoffsetはunknownとします。成功のInformationを読めるのはsaved ordinary Read/Writeかつstatus bitsが厳密に0の場合だけです。rangeがunknownでもこの分類は維持します。PnP、flush、public/internal control、special minor、非zero statusではInformationを**引数評価より前に避け**、presence=0・storage=0にします。unknownは観測された0ではありません。major0x0fはinternal/SCSI aliasなのでpublic IOCTLのunionを読まず、payloadやpointerを保存しません。

Linuxで実行できるのはhost scalar試験とsynthetic COFF checkerです。

```sh
mkdir -p experiments/stack-trace/out
cc -std=c17 -Wall -Wextra -Werror -O2 -Iexperiments/stack-trace/include experiments/stack-trace/host/snapshot_vectors.c -o experiments/stack-trace/out/snapshot-vectors
experiments/stack-trace/out/snapshot-vectors
node --test experiments/stack-trace/tools/test-object.mjs
```

`capture-build-proof.yml`は通常のhosted runnerだけを使い、checkout/upload actionはcommit固定、permissionはcontents:readです。Windows jobはD09のMicrosoft NuGet package pin・SHA-256・signature verificationを再利用し、`/c /std:c17 /kernel /GS /Zl /W4 /WX /showIncludes /O2`でAMD64 `.obj`だけを作ります。host scalar executableは通常のuser-mode試験のためにlink/実行しますが、WDK objectはlink/loadしません。platform-default packingを変更せず、`_KERNEL_MODE`も手動定義しません。

`/showIncludes`の相対パスは、compiler起動直前のPowerShell FileSystem locationをbaseにした`GetFullPath(path, basePath)`で解決します。process cwdへの暗黙依存は使いません。許可範囲は従来どおり当該runの検証済みkitと実験の`include/`配下だけです。先に17件のpath試験を実行し、成功件数をmanifestへ記録します。fixture pathは実在不要で、試験はdirectory作成やlocation変更を行いません。source/helperの存在だけではWindows実行済みとは扱いません。

同じworkflowでRust1.99.1の`cache-capture-core --release`試験をUbuntu24.04/Windows2025 runnerの両方で実行し、名前付きの非zero passとignored=0を必須にします。既存host CIのdebug/fmt/clippyとは別にoptimized buildを検査するものです。C snapshot、Rust core、Cargo/lock/toolchain、P02契約の変更をいずれもpath filterに含め、layout/semantics変更時に再検査します。Windows runnerはclient実機の代理ではありません。

公開artifactは`out/evidence/`のtext/JSONだけです。exact source/tree・source/header/compiler/object hash、commandsと各native出力、実case数、symbolチェック結果、runtime未実施のfalse flagを残します。同じ公開build manifestとheader hash JSONは通常のjob logにも表示し、artifactが取得できない場合も照合できます。環境変数や秘密情報の一括表示はしません。`.exe`・`.obj`・`.sys`や実機データはuploadしません。VS17 selectionはcompiler binary自体のpinではないため、各runのversion/hashを記録します。actual Windows compile・独立review・受入状況は[P02証拠](../../docs/evidence/P02.md)を参照し、sourceが存在するだけで成功扱いしないでください。

P02からP01 JSONへの自動変換、driver/collectorの実装、boot/署名設定変更、install/実機操作、D01/D04/D10/F08/F09/G1のgate解除は含みません。以下のlab計画はそのまま別の承認・reviewが必要です。

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

公式資料は管理者command promptを案内している。read-only queryだから非昇格でも必ず成功するとは仮定しない。権限不足では自動昇格せず、必要な限定queryと理由を示して所有者に確認する。

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
