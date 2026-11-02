# 検証戦略とリリースゲート

これは試験計画であり、合格報告ではありません。現在、製品driverの試験結果はありません。

## 1. 分離する証拠

| 層 | 検証するもの | 代わりにはならないもの |
|---|---|---|
| Rust純粋関数 | 範囲、schema、統計、決定的なモデル部品 | Windows IRP/MDL、実disk、durability |
| 状態モデル・差分試験 | invalidate/世代/公開条件、反例の検出 | 未モデル化したraw I/O、DMA、実機timeout |
| user-mode engine harness | OCF等のadapterとcache意味論 | kernel IRQL、PnP、device寿命 |
| disposable Windows VM | passthrough、stack観測、shadow、fault回復 | SSD firmwareの挙動、実電源断 |
| lab実機 | 実sector/接続/SSD、HVCI、sleep、障害、Verifier | 全機種・未試験buildでの保証 |
| HLK/署名/配布 | 公開条件・Windows trust・指定互換性 | あらゆるデータ整合性バグがない証明 |

普通の開発PCや未backupのHDDをqualification環境にしない。書込み試験は、使い捨てデータと明示的に選定された対象に限定する。原本/cacherangeのfingerprintを開始直前に再確認する。

## 2. オラクル

D02/D07が次の区別を固定し、F06で実装する。

- request submit、原本への適用、completion、flush/FUA durabilityは別event。
- write完了済みでも、通常のvolatile device cacheに残る場合は電源断後の保持を無条件に要求しない。
- 重なったI/Oに原本が保証しないmulti-sector atomicityを仮定しない。
- TRIM/UNMAP後はunknown。再writeするまでゼロ埋めを期待しない。
- 先に完了したwriteの後で開始したreadに古いcacheを返す、early ACK、dirty cache依存、過去sessionのpublish/slot破壊は失敗。
- cache errorとorigin errorを別々に注入し、後者をcacheで隠して成功にしない。
- modelと製品coreが同じロジックを丸ごと共有して、同じ誤りで一致する試験にしない。

各traceはschema version、seed、explicit completion order、期待されるbytesまたは許容集合/unknown、期待するcache状態を含める。traceの「期待値」は上位レビューで固定し、実装結果に合わせて変更しない。

## 3. 必須回帰ケースと担当

| ID | 入力/順序 | 必須の結果 | owner |
|---|---|---|---|
| T01 | cold read → fill → read | 原本と同じbytes、hit/missを識別 | F07 |
| T02 | cached line内partial write | 交差line失効、更新後readが一致 | F07 |
| T03 | 4095から2bytesのwrite、4KiB line | 2 linesとも失効 | F01,F07 |
| T04 | read A → write B完了 → late fill A | Aのpublish拒否、後続read B | F07 |
| T05 | 重複writer 2個、一方だけ完了 | もう一方が完了するまでunsafe hit/fillなし | F07 |
| T06 | cache timeout → HDD retry → late SSD complete | caller完了後buffer書換えなし、二重completionなし | F08,W03,Q01 |
| T07 | epoch変更後、旧SSD writeが遅延完了 | 同じ物理slotの早期再利用なし | F08,W04,Q01 |
| T08 | cancel request → late complete | cancelを完了と誤認せず、参照が最後に回収 | F08,W03 |
| T09 | DSM/TRIM/offload/raw mutation | 正しいinvalidate/quiesce/拒否、以後stale hitなし | D01,W06,Q01 |
| T10 | flushが保留中writeに追いつく | 原本barrierの順序を維持、early ACKなし | D07,W05,Q01 |
| T11 | discard後read | zeroを捏造せずoracleはunknown | F04,F07 |
| T12 | cache corruption/error/full/readonly | invalidate/fallback、原本結果をpreserve | W04,Q01 |
| T13 | origin error | 古いcache/ゼロで隠さない | W05,Q01 |
| T14 | target交換/ID再利用/resize | target拒否またはcold reset、誤disk操作なし | C05,W07,Q01 |
| T15 | sleep/hibernate/online/disable + in-flight | old I/O quiesce、cold再開、不能ならdegraded | W07,Q01,Q02 |
| T16 | low memory/queue full | 上限内、bypass/定義したerror、リークなし | W03,Q01 |
| T17 | service crash/GUI crash | data pathがservice待ちで停止しない | C04,U02,Q01 |
| T18 | malformed/unauthorized control | 拒否、write権限昇格/情報漏洩なし | C06,Q01 |
| T19 | no target configured / unsupported disk | 意図しないcache化/boot障害なし | W01,W07,Q02 |
| T20 | old driver/config upgrade/rollback/uninstall | 原本保持、無効化/回復手順成立 | C08,Q02 |
| T21 | FFI invalid enum/len/panic/callback order | UBや二重freeにせず契約どおり処理 | D04,W02,Q01 |
| T22 | unknown BypassIO path | supportを偽らず診断可能なveto/bypass | D01,W06 |

D01が捕捉できない経路を見つけたら、試験を消さずサポート境界を狭めるか方式を変更する。重複・late completionは注入可能でなければ「未検証」。

## 4. Gate acceptance

### G1 — 設計/モデルの凍結

D01〜D10の出力（ADR、trace、所有権、ABI、portable oracleとWindows mapping）が相互整合。B03の初期資源上限が承認済み。F01〜F08の必要試験とF09のengine対照試験が合格。意図的にstale-fill、early ACK、late slot再利用の故障modelを作ると、必ず対応試験がFAILする。安全契約INV-01〜12がIssue/試験へ対応。後続kernel作業へ渡すheader/schema/limitsに未決の「都合よく決める」箇所がない。

D03のOCF採否とD09のkernel Rust採否は別判定。No-Goなら代替ADRと後続Issueを更新し、勝手に既存Issueを実装しない。

Pre-G1のD01/D09 probeとD03 feasibility spikeはexperiments配下の隔離実験で、W01製品driverではない。D01/D09のWindows probeはcache hit/fill/raw cache writeを実装しない。D03のhost spikeは模擬cacheの動作だけを検証できる。compile/資料/模擬/実Windows証拠を分ける。install、mutation生成trace、Verifier、VM/実機実行は別の明示lab承認が必要。B03はhost測定と保守的仮定で初期上限を決め、後段のW03/W09実測を先取りの完了条件にしない。D08のgovernance/配布実現性gateは意図的に維持し、未決承認でW01を解禁しない。

### G2 — Passthrough / Shadow

W01〜W08のうちshadowまで、W10のdriver側control認可、C04〜C06が使い捨てWindows環境で動作。cacheからアプリへデータを返さず、原本baselineとの結果・error・順序を比較。比較は同じ世代のデータのみ。同時writeで内容が変わる正当な差を誤検出しない。

no-target boot、cache消失、service停止、拒否構成、管理操作failureで原本を壊さず回復する。基礎traceやDriver Verifier計画がそろっていなければ、cache hitを有効にするbranchへ進めない。

### G3 — Lab hit-serving

W09を有効にした使い捨てlab構成でT01〜T22の該当ケース、stress、低memory、cancel/timeout/remove、復旧、cold再起動を実証。データ相違・二重完了・use-after-free・持続的resource漏れはゼロ。下位driverの永続hangを有限時間で回復できるとは主張しない。観測不能なケースは未検証として残す。

B03で先に承認したmemory/inflight/queue上限と許容overheadを満たす。性能のためにintegrity testを無効化しない。

### G4 — 限定betaの可否

Q01〜Q03、D08、GOV01、C08（C07を含む）が完了し、独立した人間のWindowsストレージ専門家が証拠をレビュー。Q04がこのG4判定を記録する（Q04自身の完了を事前条件にしない）。対応build・WDK・firmware・sector pair・stack・既知非対応を明示。署名/HLK・HVCI・Secure Boot・更新/rollback/uninstallの証拠と利用者向け警告を添付。

ライセンス/通知、脆弱性受付、責任者、署名主体、公開承認が未決なら公開しない。issueにAIレビュー済みとあるだけで人間レビューを完了扱いしない。

## 5. 性能評価

比較するのは同じhardware、working set、repo snapshot、OS cache条件、試行回数。SSD cold/warmとOS cold/warmを混同しない。

- 安全なsynthetic workload: small-file read、random 4/16/64KiB read、sequential scan、read/write混合、hot setがRAMより小/大。
- 実作業: 同じrepoの検索、初回/再index、dependency読み込み、build等。ネットワークdownload時間をstorage改善として計上しない。
- baseline: 未装着HDD、installed-but-disabled/passthrough、shadow、active cold、active warm。
- metrics: wall time、p50/p95/p99、CPU、nonpaged peak/steady state、HDD/SSD bytes、driver到達I/Oでのhit率、SSD書込量、miss時overhead。
- B03が採否の閾値と反復数を測定前に承認する。結果を見て有利なケースだけ残さない。
- working setがRAM cacheに収まって改善が小さい場合も正直に報告し、SSD直置きとの複雑性/費用を比較する。
- cache reset/OS cache制御等の実験操作はlab限定。普段のPCへ強制的なcache purge/driver installを行わない。

## 6. 証拠の保存

各実験は `evidence/<gate>/<run-id>/`（planned path）にmanifestと再現コマンド、seed/trace、結果、artifact hashを保存。実disk内容/個人情報を含むdumpは公開repoへcommitしない。

必須metadata: source commit、toolchain/WDK、OS build、匿名環境ID、接続/sector pair、mode、設定予算、試験ID、seed、合否、未検証事項、reviewer。機器serialやユーザーpathは診断allowlist外。失敗も保存し、修正後に同じseedで再実行する。

## 7. CIと人間の作業

PR CIはhost Rust tests / fmt / clippy / schema & docs consistencyだけ、contents:read、秘密なし。driverのbuildは独立job。self-hosted lab、Verifier、実機の障害注入、signingはtrusted maintainerの別workflow/手順とし、fork PRから起動しない。

Driver VerifierはOSを停止させることがある。driver開発専用VM/実機だけで使う。HLKの正確なplaylist/要件は対象Windows releaseごとに再確認する。

