# VM-AのT1限定でP04を観測し、R1へ戻す手順案

2026年11月19日 JST。VM-Aの所有者、ゲスト作業者、ホスト側の復旧担当、独立レビュアー用。基準は main `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c` と匿名化済みの `AFTER_R1` 報告。

**実行判定は保留（NO-GO）。** VMと復元点の準備は完了した。対象限定の登録手段、現在の信頼設定に適合する配布物、異常時に収集プロセスへ停止を伝える仕組みがそろっていない。本書では将来の操作と合格条件を定める。今回行うのは手順の机上レビューまでである。実際の登録・署名・ロード・観測・VM変更は行っていない。[引き継ぎの工程2〜4][handoff]・[P04の受入範囲][readme]

**異常時に開く箇所**　対象・署名・stack・権限のどれかが不明なら観測を開始しない。開始済みなら第5節で停止の状態を記録し、第6節でホストから復元する。ゲストが応答しない場合は、第5節の最終行を参照する。以下でD01はゲストのOSディスクの匿名名を指し、課題番号には「D01 #3」と付記する。

## 1　今回の判断に使える実測

| 対象 | 取得済みの事実 | 次回との照合点 |
|---|---|---|
| ホストのVM-A | 所有者がWorkstation上で、正常停止時に取得したR1へ復元。R1はR0の子。OSとT1の両方の差分ディスクが正しい元VMDKにつながり、復元前後で参照先が変化。正常起動を確認 | 復元の単独完了ログは未取得。VM名や「現在位置」表示だけに頼らず、非公開のVM・R0・R1の識別記録、親子関係、両ディスクの連鎖と稼働状態を使う |
| 既存OSディスク D01 | `nvme0:0`／LUN 0、GPT・5区画。EFIはP1、OSと自動・小メモリダンプ保存先はP3。復元後の稼働中pagefileはP3、2,432 MiB、`TempPageFile=False` | 接続前のpagefileは同じD01のP5、6,144 MiB。移動原因は未確定。D01を常に対象から除外する |
| 観測対象 T1（ゲスト表示D02） | Workstationで作成した データ用VMDK、`nvme0:1`／LUN 1、通常モード。8 GiB・RAW・区画0。OS、boot、稼働中pagefile、dump、専用dump、休止用途に不使用。専用dump設定なし、休止は非対応 | 領域Aには別に未接続の旧 候補VMDK がある。復元後はスナップショットの子VMDKをたどる。ファイル名・容量・ディスク番号・ドライブ文字の単独一致は採用しない |
| ゲストとstack | Windows 11 Pro 26H2。接続後 `26300.9711`、復元後 `26300.9974`。両ディスクのIDとシリアルは各1件一致。共通の親は `stornvme`、ゲストPCI 19/0/0。disk自身の `LocationPaths` は空、親の位置と各LUNは取得済み | 実stackは上から `partmgr → disk → stornvme`。`EhStorClass` は下位クラス登録で、両ディスクの実stackには表示されていない。volume側の `volsnap / iorate / fvevol`、登録のみの `rdyboost`、dumpの `dumpfve.sys` も別に記録。P04名に一致する登録・ロードは0件 |

ゲスト欄の根拠は所有者が中継した接続前・接続後・R1復元後の照会。ホスト欄はVMware構成・スナップショット・稼働の照合記録による。試行対象buildは `26300.9974` とし、R1取得時の正確なbuildや更新時点は推定しない。次回のbuildが違えば記録を更新し、適用範囲を再判定する。

## 2　登録前に、実行担当とホスト担当がそろえるもの

1. **所有者が操作別に許可する。** 対象VM・T1・build・一度の観測窓、署名済み一式の持ち込み、ゲスト内での登録、起こり得るPnP再起動とロード、観測、証跡の退避先、正常終了とホストからのR1復元を記入する。登録時にPnPが作動し得る経路は、その時点でロードまで許可されている必要がある。ホストの管理権限拡大、物理ディスク、クラス全体の登録、既存filterの削除、初期化、書込み試験、Verifier、信頼設定の緩和は対象外。
2. **バイナリを確定する。** 現mainにはインストール用INF・カタログ・登録ツール・配布用バイナリがない。CIでリンクした未署名AMD64イメージのSHA-256は `9fcd5d601d5c17ce0b48bd03e6b08f92ce02645056b3686d738337949de0077b`。CI成果物にあるのはテキストとJSONだけ。将来の担当は、ソース、WDK・コンパイラ、署名前イメージ、実際のINF・CAT・SYS、登録ツール、live機能付きcollector・証跡受信側の版とハッシュを対応付ける。署名後の実ファイルはそのハッシュで別に固定し、カタログへの包含・署名チェーン・ゲストの実効Code Integrity方針で許容されることを確認する。Secure Boot・HVCI・署名方針の状態が不明、または既存設定で検証できなければ停止する。[ビルド証拠][build]・[署名方針][sign]・[HVCIとの互換性][hvci]
3. **ホスト担当が復元を引き受ける。** 実行中のゲストに依存しないWorkstationコンソール・必要な認証・R1の識別を手元で確認。両VMDKの通常モードと連鎖、作業先・差分保存先の当日空き、R1以降に失われる変更と退避先を記録する。過去の空き346 GiBを当日値に流用しない。私的なホスト記録はR1の外へ保存する。ゲストが正常終了できない場合の扱いは第5節で事前合意する。
4. **ゲスト担当が2台を新規列挙する。** `Get-Disk`、`Get-Partition`、`Win32_DiskDrive`、`Win32_PageFileUsage`、dump・休止設定、`pnputil /enum-devices /class DiskDrive /stack /services /drivers` 等を用いる。非公開の個体記録（storage ID・シリアル・完全なPnP instance ID）、親の位置・LUN、ホストの連鎖を突き合わせ、D01とT1が別々に各1件になることを確認する。boot/system、全pagefile、dump・専用dump、休止を物理ディスクへ対応付ける。P04は新たな特殊用途への `InPath≠0` 通知を拒否する。既存の特殊用途は遡って検出できないので、ゲスト割当て等も含め対象上の特殊用途を除外できなければ開始しない。値の欠落・照会拒否・ID変化・二重一致は「確認不能」とする。[対象と特殊用途の契約][selectorcontract]
5. **基準を保存する。** D01・T1のpresent/started状態、各デバイスおよびDiskDriveクラスのUpper/LowerFilters（未設定・空・型・値・順序）、有効なstack、サービス・パッケージ、volumeとdumpのfilterを個別に記録。値を取れない状態を空のリストと解釈しない。登録ツールの予定変更一覧を非適用モードで確認し、対象devnodeがT1だけ、対象外への変更0件でなければ停止する。公開可能な記録には匿名ラベルと差分だけを使う。

## 3　T1だけに登録する方式案と、装着後の受入条件

**採用候補は、パッケージ・サービスの用意と、T1のデバイスインスタンスへのfilter登録を分ける方式。** 前者にはゲストのDriver Store・サービスへの変更が含まれる。後者が書き換えてよいデバイスはT1だけである。mainにはこの登録手段の実装・受入証拠がないため、下記を別のソース／ビルド審査で成立させるまで実行しない。

1. 既存の `disk` 機能ドライバーを保持する。実験専用のカーネルサービスは要求時起動とし、パッケージの配置、サービスの準備、対象IDの確定、T1への登録、最初のロードを個別に追跡できる案とする。起動時の自動ロードや、パッケージの配置を契機とする即時ロードを許す案はここでは採用しない。パッケージの配備だけで他デバイスの再構成や自動開始が起きないこと、既存サービス・同名オブジェクトと衝突しないことを、最終INFと全付随セクションで審査する。T1を一意に選ぶ前には有効化しない。
2. ゲスト内の登録ツールが、非空の**完全なinstance ID**で `SetupDiOpenDeviceInfoW` を使い、現在presentなDiskDriveのT1であることを再照合する。ドライバーサービスの `Parameters` キーにある `TargetInstanceId` 値へ、同じIDを `REG_SZ` として設定し、読み戻す。最大512 UTF-16コード単位（終端NUL込み）、途中NUL・切り詰め不可。これを**最初のロードより前**に完了する。ドライバーは `DriverEntry` で一度だけ値を読み、`AddDevice` でPDOのIDと大文字小文字を区別せず完全一致したときだけ装着を試みる。不一致や装着準備失敗でもPnPに成功を返す場合がある。サービス開始成功だけでは装着確認にならない。[対象照合実装][driverselect]・[装着失敗時の処理][attachfail]・[特定インスタンスの選択][openinstance]
3. **変更先をT1の `SP_DEVINFO_DATA` に固定する。** 実装候補は `SetupDiSetDeviceRegistryPropertyW` の `SPDRP_UPPERFILTERS`。これはインスタンスのサービス名リストに対する操作で、Windowsが最終的に組み立てるstackとは別の値である。現在値・型を取り直し、既存の文字列・順序を保ったまま専用サービス名を1件だけ追加する案とする。正しい元状態と完全一致を検査し、他のインストーラーとの競合を排除できない、意図しない値・既登録・重複・途中失敗・再試行の状態を判別できない場合は中止する。登録直後には対象IDとリストをもう一度取得し、型・既存順序・追加した1件が予定差分と一致することを確認する。読み直しだけでは競合を防げないので、実装側で排他を実証できない場合は適用しない。部分適用時は最後に確認した状態とエラーを記録し、R1へ戻す。古いリストを上書きする修復は対象外とする。署名済みサービスの配備・保守とこの方式の組合せが対象buildでサポートされるかは**未立証**。[対象プロパティ][prop]・[設定APIの対象と制限][setprop]
4. Windowsの標準的な `DDInstall.Filters / AddFilter` は候補比較に使う。Extension INFのモデル照合はhardware/compatible ID単位。同じコントローラーを使う2台のPnP instance IDが別でも、INFに同じIDが適合する可能性がある。現時点で両台のhardware/compatible IDの非重複は証明されていない。汎用ID・DiskDriveクラスへの適用、OSディスクを含む一括install、クラスUpperFiltersの編集、disk.infの置換を選ばない。インスタンス限定案が成立しなければ、代替方式を別レビューへ戻す。[Windowsのfilter登録][installfilter]・[Extension INFの照合][extinf]
5. 承認されたT1のPnP再始動／ロード境界で、直前・直後にIDと関連サービス・プロパティ・実stackを再列挙する。D01や親コントローラーの再始動は対象外。ゲスト全体の再起動が必要と判明した時点で止めて、別の許可を求める。本案で観測対象にする配置は上から **`partmgr → P04 → disk → stornvme`**（T1の`disk`直上）を候補とする。現物での成立は未確認である。T1にP04が1個、既存要素は同じ順序、D01は元の3要素のまま、クラス・他デバイス・volume・dumpの差分は0件であることが必要。PnPUtilの表示で当該境界を識別できない場合も不合格。同レベル内の順序を決め打ちしたり、`partmgr` や `EhStorClass` を移動・削除したりしない。境界が違えば `BEGIN` に進まず、復元後に方式を見直す。[ADR-004][adr]・[Windowsのフィルター順序][ordering]

**登録ツールの受入に必要な例。** T1のみ選択できる正例に加え、同型2台のhardware IDが同じ、対象IDが空・古い・長過ぎる、対象消失、D01選択、既存filterの並行変更、selector設定後やリスト書込み後の中断、二度実行を別件の固定試験にする。誤適用0件、元のfilter保持、拒否・部分適用の識別を確認する。これらは未実施の受入試験である。

## 4　観測の前にアクセス権と証跡の行き先を確認する

1. Object Managerで制御デバイス（CDO）の実オブジェクト `\Device\WinSsdHddCacheProbeP04` を指定し、アクセス制御リスト（DACL）を取得する。パスの先頭と `Device` 直後のバックスラッシュは各1文字。候補は既存・承認済みのAccessChkによるObject Manager名前空間（`-o`）の完全な記述子（`-l`）の照会。対象をDeviceとして確認し、読めない場合はこの段階で止める。既定の許可対象はLocalSystemと組み込みAdministrators。専用クラスGUIDの上書き設定、リンクが指す実体、許可／拒否ACEを照合する。サービスやシンボリックリンクだけのACLでは、この確認を完了できない。[AccessChkの表示対象][accesschk]・[CDOの実装][cdosource]
2. 開いているハンドルがない状態で、標準ユーザーおよび昇格していない管理者トークンで、読取りのみ・書込みのみ・collector相当の読書きopenを順に試し、**アクセス拒否**を確認する。所有者が許可した昇格管理者では、同じエンドポイントをcollectorと同じ読書き権限で開けることを別に確認する。確認用の各ハンドルを閉じる。この段階でIOCTLや `BEGIN` は送らない。競合による共有違反・デバイス未準備・未検出は、ACLによる拒否の証拠に数えない。DACLの取得不能・想定外の権限・所有者を特定できない場合は停止する。[CDOの契約][cdo]・[既定セキュリティ記述子の上書き][securedevice]
3. **最初の試行案は1回・1,000 msの受動観測だけ。** T1はRAWのままとし、実験側からストレージI/Oを生成しない。通常OS動作に由来するI/Oを0件と保証するものではない。ファイル作成、raw書込み、初期化、lock/dismount/offline・抜去試験を加えない。専用collectorは `experimental-live` を有効にした実物と明示スイッチが必要。時間は本案の値で、実行前に所有者と確定する。
4. ホスト担当は監視と復元をゲストとは別に行う。collectorのstdoutは**バイナリを保持するパイプ**とし、下流の低権限受信側が、承認済みのR1外の非公開保管先に証跡を退避できることを先に検証する。既知の合成archiveで元と受信済みファイルの長さ・ハッシュ・オフライン解析を照合する。データディスクT1への保存、文字列変換、コンソールや通常ファイルへのstdout直結は使えない。生のPnP情報とarchiveは分けて私的に保存し、バイト数・SHA-256・ゲスト／ホスト時刻・binaryの版を対応付ける。正常でもcollectorはarchiveを**終了時**に出力する。有効な出力を発行したときの `collect`／`audit`／`project` の終了値は `4`。受信側での保存成功、記録の完全性、観測結果の妥当性はそれぞれ照合する。[collectorのCLI][cli]・[収集とアーカイブの契約][collectcontract]

## 5　異常時に誰が何を止めるか

ゲスト担当は新規ワークロードと追加の試行を止める。ホスト担当はゲストの外で応答と経過時間を監視する。初回案ではcollector起動から15秒で未終了なら異常と判定する。この15秒は運用案で、実測した保証時間ではない。一度のロードにつき一つのセッションであり、`STOP` は観測の新規受付を終える操作で、driverのdetach／unloadとは別である。[所有者と状態遷移][cdo]・[実装した収集ループ][loop]

| 発生時点・症状 | 実行時に取る処置 | その試行の記録 |
|---|---|---|
| `BEGIN` 前に別対象・特殊用途・stack・ACL・署名の不一致 | `BEGIN` を送らない。変更前なら作業終了。配備や登録が始まっていれば第6節へ | 未開始。途中まで書かれた対象だけを記録 |
| 通常の時間満了・収集量上限 | 同じFileObjectを持つcollectorが `STOP`。成功後だけ、規約に従い残件を `DRAIN`。`Sealed / active=0 / queued=0` と終端を照合する。ホスト監視は続ける | 追跡した記録の終端。実際の全IRPの完了や正常unloadの証明には使わない |
| `BEGIN` が失敗／返答不明 | 現実装はその場で収集を打ち切る。開始成立を断定せず、同じロードでBEGINをやり直さない | アーカイブなしを含む開始不明。必要ならR1へ |
| 成立後のDRAIN／応答検証に失敗 | 失敗したDRAINを再送しない。同じownerで新しいcall IDのbest-effort STOPを規約の範囲で1回だけ試す。正常なSTOP自体が失敗した場合も、新しいcleanup STOPが許される場合に限る。失敗後の追加DRAINをしない | 未完了。疑わしい応答や失われたレコードを補わない |
| 外からの中止要求、誤ったI/O・他デバイスへの影響、ゲストの異常 | 担当が新規ワークロードを中止。現行CLIには外からのstopサブコマンドや、所有者への中止通知入力がない。2本目のcollectorや別ハンドルでは停止できない。要求を**同じ所有者**に渡し、次の安全な境界でSTOPする方法を別途実装・固定試験するまで、外部異常時の早期停止を要する試行は保留 | kill、Ctrl+C、CLEANUPを確認済みSTOPや終端と数えない。ゲストが応答すれば、停止確認の有無を記録したうえで正常終了し、第6節へ |
| 同期IOCTLが戻らない、ハング、起動不能 | 5,000 msはSTOP後のユーザーモード側ポーリング上限であり、ブロック中の呼び出しを中断しない。事前合意したホスト監視期限で「停止未確認」としゲストへの指示・再試行を終了。正常終了可能なら第6節へ。不能なら所有者に引き渡す | 未完了。ホストからの強制停止・稼働中の復元は既存の承認外。例外回復の手段・許可が未合意ならハングを伴い得る実験を開始しない |

欠落・skip・未知counter、時間／容量上限、予期しないPnP・電源・特殊用途通知、証跡出力失敗があれば次の試行を禁止する。現行collectorは各producer faultやskipを直ちに停止のきっかけとして監視しない。異常を検出した瞬間に止めることが要件なら、返答のfault・counterと所有者からの中止通知を確認する位置、保留中の同期呼び出しが戻った後のSTOP、ホスト監視との分担を定めて先に試験する。[collectorの継続条件][drainstate]オフラインで `audit`／`project` を実行し、完全な応答フレーム・footer、対応するsubmit/completion、欠落数、終了コード、受信側の実バイト列を照合する。0件は `EMPTY_CAPTURE`。終端や `retained_records_reconciled` が得られても、WindowsのI/O網羅、ディスク上の正確な範囲、D01の合格は未検証である。[archiveの判定規則][audit]・[I/O対応表][coverage]

## 6　ホストからR1へ復元する

1. 初回の試行は正常終了時も証跡退避後にR1へ戻す。異常時は再開や手作業のfilter除去を止める。ゲストが応答する間に、退避可能なarchive・監査結果・関連する署名／PnP／障害イベントとホスト時刻をR1外に確保する。退避可否と欠落を記録する。証跡が取れないことを理由に、安全のための復旧を無期限に待たない。
2. ゲストを通常シャットダウンする。ホストで管理対象がVM-Aであること、稼働一覧から外れ、正常終了・非サスペンドであることを照合する。正常停止できないときは第5節のホスト判断へ戻る。R1以降の変更が失われる範囲を所有者が了承済みであることを照合する。
3. **ホストの**Workstationのスナップショットマネージャーで、個別にR1を選択して「移動／復元」（Go To）を実行する。R0はR1の親。表示名だけで選ばず、準備時のR1識別記録・作成時点・親R0・両ノードの元VMDK・状態ファイルを確認する。不一致、ロック、認証や容量の不足ならエラーを記録して中止する。VMwareの暗号化パスワードは所有者がホストの専用画面へ入力する。R0の選択・削除・統合、VMDKの手動編集は行わない。
4. ホストで復元操作の結果・時刻、現在位置R1、R0保持、両ディスクの差分から元VMDKまでの連鎖、コントローラー・部分暗号化・vTPMを照合する。差分VMDK名は変わり得る。現在位置表示だけを復元実施の証拠にしない。停止時のR1へ戻れたら通常起動し、稼働一覧とゲストの起動を確認する。
5. ゲストで第2節の2台を新たに同定する。T1は8 GiB・RAW・区画0で特殊用途に不使用、D01がEFI/OS/pagefile/dumpを保持、既存stack・フィルター登録は基準どおり、当該パッケージ／サービス／ロード・P04装着の痕跡は基準どおり0件かを照合する。ディスク番号・PnP IDの再利用を仮定しない。build、pagefileの所在・割当・`TempPageFile`、証跡退避の成否も記録する。差分は修正せず分類し、原因不明の対象・隔離・復元異常が残れば試行を終了する。R1復元済みでもbuild同一性は別途確認する。

## 次の審査に渡す判断

| 判断項目 | この案の判定 | 実行保留を解除するための材料 |
|---|---|---|
| VM-A・T1・R1の準備 | 記録上は適合 | 実行直前の再同定とbuild照合。準備済みR1を保持 |
| 配備・登録・順序 | **保留** | 署名・配備の責任者と実物、T1限定の登録手段・保守／競合条件・負例試験、期待する装着位置と実効ACLの検査手段 |
| 異常停止・証跡・回復 | **保留** | 同じ所有ハンドルで中止要求を受ける仕様と固定試験（開始前後の競合、DRAIN失敗、STOP確認不能、同期呼び出し停止）、ホスト監視期限、正常終了不能時の権限境界、私的なR1外保存先と受信検証 |
| 実験・製品の判定 | 実行許可なし | 独立レビュー後に必要な差分だけをソース／ビルドで検証し、所有者が操作別に判断。D01 #3は実際の経路ごとの証拠待ち |

判断に迷う場合は変更を止め、対象・観測できた状態・欠けた証拠を所有者とストレージ担当に返す。本書の独立レビュー結果は同じフォルダーの [独立レビュー記録](P04-independent-review.md) を参照する。

[handoff]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/handoff/2026-11-03-windows-next-step.md#L20-L25
[readme]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/README.md#L45-L66
[build]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/evidence/P04.md#L338-L362
[sign]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-policy--windows-vista-and-later-
[hvci]: https://learn.microsoft.com/en-us/windows-hardware/drivers/driversecurity/implement-hvci-compatible-code
[selectorcontract]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/contracts/upper-candidate-probe-v1.md#L22-L50
[driverselect]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/wdk/driver.c#L21-L125
[attachfail]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/wdk/driver.c#L143-L183
[openinstance]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendeviceinfow
[prop]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-upperfilters
[setprop]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw
[installfilter]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/installing-a-filter-driver
[extinf]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/using-an-extension-inf-file
[adr]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/adr/0004-interception-boundary.md#L11-L26
[ordering]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/device-filter-driver-ordering
[cdo]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/contracts/upper-candidate-probe-v1.md#L84-L98
[accesschk]: https://learn.microsoft.com/en-us/sysinternals/downloads/accesschk
[cdosource]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/wdk/driver.c#L15-L19
[securedevice]: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdmsec/nf-wdmsec-wdmlibiocreatedevicesecure
[cli]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/main.rs#L28-L107
[collectcontract]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/contracts/upper-candidate-probe-v1.md#L191-L209
[loop]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/capture.rs#L98-L170
[audit]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/contracts/upper-candidate-probe-v1.md#L248-L262
[drainstate]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-core/src/archive.rs#L472-L502
[coverage]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/contracts/io-coverage.md
