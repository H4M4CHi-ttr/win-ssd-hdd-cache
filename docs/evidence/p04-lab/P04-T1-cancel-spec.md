# P04　T1限定登録とcollector中止通知のソース・ビルド検証仕様

2026年11月19日 JST。対象は実装担当と独立レビュアー。基準mainは `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c`。元の[レビュー済み手順案](P04-VM-A-T1-runbook.md)のSHA-256は `37f8c86ec02576cc4ef703d09a01a5112c6b9a326155def4bf41d6d5e0ac1451`。その[独立レビュー記録](P04-independent-review.md)、[本仕様の試験表](P04-T1-cancel-test-matrix.md)、[本仕様の独立レビュー記録](P04-T1-cancel-review.md)を併用する。

登録ツールは、保護するOSディスクと対象T1を別々に同定でき、読取り結果からT1だけに変更範囲を限定した計画を出す。競合する書き手や再構成を排除できる証拠がなければ適用前に停止する。中止通知は、現在のcollectorが次に制御を取り戻した時点で受け取り、同じハンドルで観測を止める。同期I/Oから制御が戻らない間は、監視側が「停止未確認」と記録する。

これは**将来の実装と受入試験の仕様**である。本日実施したのは文書と根拠の照合・レビューのみ。今回定義したケースは全件未実施で、実装・ビルド合格および実機実行の承認はない。

## 判断の基準

| 既知の事実 | この仕様への反映 |
|---|---|
| VM-AのR1はR0の子。停止中にホストからR1へ戻し、OSとT1のVMDKの連鎖と起動を確認した。ゲストは復元後 Windows 11 Pro 26H2、build `26300.9974`（接続後は `26300.9711`） | 試験用データでは匿名値を使う。buildの同一再現、P04装着時の回復、正常終了できないときの復元は未実証 |
| ゲストのD01はOSディスク。`nvme0:0`／LUN 0、EFIはP1、OS・dump・pagefileはP3。pagefileは2,432 MiB、`TempPageFile=False`。D02＝T1は `nvme0:1`／LUN 1、8 GiB、RAW、区画0で特殊用途に不使用 | 両台は同じ `stornvme` コントローラー。ディスク自身の `LocationPaths` は空でも、親位置・LUNと私的な複数の識別子で各1台に対応した。番号・容量・ハードウェアIDだけで選ばない |
| 既存の実stackは `partmgr → disk → stornvme`。`EhStorClass` は別に記録された下位クラス登録。今回の対象はVM内から作成した データ用VMDK。未使用の旧 候補VMDK もホストに残る | プロパティ上の登録順と実stackを別に扱う。ホストの元VMDKまでの連鎖を確認し、旧ファイルを選ばない。課題「D01 #3」とディスクの匿名名D01を区別する |
| mainの `DriverEntry` はサービスの `Parameters` キーの `TargetInstanceId` 値を一度だけ読む。`AddDevice` は完全一致するPDOへの装着を試み、対象外や準備失敗でも成功を返し得る | ドライバー内の選択とWindows側の登録対象を別々に検証する。登録、ロード、装着、観測、全I/Oの捕捉には、それぞれ別の根拠が必要。[対象照合][selector]・[装着失敗時][attach] |
| 現行 `collect` はWindows＋明示feature＋スイッチでのみliveへ進む。同期 `DeviceIoControl` を使い、`BEGIN / DRAIN / STOP / STATUS` は同じFileObjectが所有する | 外部へCDO、session IDによる操作権、STOP用IOCTLを新設しない。driver・wire・native archive v1・既存投影の形式を維持する。[CLI][cli]・[Win32境界][win32]・[所有者契約][owner] |

## R　登録の入力、変更範囲、失敗時の扱い

**REG-1　計画は常に読取りから始める。** 許可したVM、ゲストbuild、R1、対象T1の私的なinstance ID・storage ID・シリアル・親の位置とLUN・ホストVMDKの連鎖を入力にする。別の保護記録からD01のID、EFI・OS・全pagefile・dump／専用dump・休止・その他の特殊用途の所在を取得する。一貫した一つのデバイス記録でT1が1件、D01が別の1件、双方presentであり、使用中のdisk classと実stackが想定どおりでなければ拒否する。観測不能は「不使用」に変換しない。T1の番号変更は許容できるが、ID・連鎖の不一致を番号や名称で補わない。

空・埋め込みNUL・サイズ超過のIDをWindowsへ渡さない。selectorは終端NUL込み最大512 UTF-16コード単位の `REG_SZ`。途中NUL、奇数バイト、末尾NUL欠落、切り詰め、部分一致を拒否する。意図しないtrim・正規化・ワイルドカード処理を入れない。Windowsの大文字小文字を区別しない完全一致と照合する。512単位はP04のselector形式自体の上限で、Windowsが受け付けるデバイスID長を保証しない。空IDを `SetupDiOpenDeviceInfoW` に渡すとルートが選ばれ得るので、API呼出し前に拒否する。[selectorの形式][selector]・[Microsoftの対象選択仕様][open]

**REG-2　変更可能な範囲を明示する。** 以下はfakeで観測する効果の名前であり、操作コマンドではない。計画には対象の世代、動作前のプロパティ、パッケージ／サービスの識別・ハッシュ、順序、変更後の期待値を固定する。私的なIDをそのまま成果物に含めない。

| 順番 | 許す効果 | 読み戻す値・停止条件 |
|---|---|---|
| P　パッケージ配置 | 審査した実験用パッケージ1件のDriver Store配置のみ | INF・CAT・SYSの組、版、ハッシュ、対象OS向けの全セクションと参照先、配布対象を照合。配置時の自動install・再バインド・ロード、D01に適合して作用する拡張INF、未審査のコピーや副作用があればPの前に拒否 |
| S　サービス準備 | 当該パッケージの要求時起動カーネルサービス1件のみ | 同名の別物、機能ドライバー置換、boot/system開始、自動起動、ロード済み／状態不明を拒否。新規作成・既存競合・結果不明を区別 |
| K　selector確定 | そのサービスの `TargetInstanceId` のみ | 型・長さ・実値を読み戻し、現在のT1との完全一致と未ロードを確認。ここまでに初回ロードが発生する経路は不採用。ロード後に値だけ直して続行しない |
| F　T1限定登録 | T1の `SP_DEVINFO_DATA` が指すdevnodeの `SPDRP_UPPERFILTERS` のみ | 同じdevnodeを再確認し、元のサービス名列の順序・表記を保ってP04名を末尾に1回だけ追加する案。D01、全クラス、他devnode、LowerFilters、既存機能ドライバーとサービス、VMの構成に対する変更要求は常に0件 |
| V　照合・終了 | 変更なし | T1の期待値、D01とクラス等の基準値、サービス／selectorを読み取る。結果は「登録の読戻し一致」まで。PnP再始動・ロードは別承認で、ソース／ビルド対象に実操作経路を置かない |

インスタンス用UpperFiltersは名前のリストである。`Missing`、正しく終了した空リスト、既存リスト、取得不能を区別する。空の `REG_MULTI_SZ` は空文字で列を終端する。型違い、奇数長、終端欠落、途中の終端後にデータ、照会失敗は拒否する。他サービスを並べ替え、重複排除し、消してから作り直す処理を禁止する。元の列にP04がすでにある場合はREG-4の再開条件で判定する。一致し得る汎用hardware IDをWindows登録の個体指定として使わない。[プロパティ][upper]・[値形式][multi]・[拡張INFの照合][extension]

**REG-3　排他は事前条件であり、再読で代用しない。** `SetupDiSetDeviceRegistryPropertyW` には期待旧値を渡す引数がない。別installerが最終比較と設定の間にfilterを追加すると、その値を消したあとでもこちらの読戻しは一致し得る。独自mutexは他の管理ツールやPnPを排除できない。対象の消失／再列挙、別の書き手、初回ロードの先行を防ぐ根拠を、**最初の副作用前からVの読戻し完了まで**要求する。各効果の呼出し直前に保護記録・対象devnodeの世代・排他の有効性を再確認する。途中に根拠を失えば次の変更を発行しない。パッケージの実際のAPIが段階を不可分にまとめる場合、その副作用が既知の順序で分離・検証できるまでPへ進めない。排他と準備済みパッケージの安全な配備を実Windowsで示せていない現段階では、実バックエンドのapplyは到達不能とする。機能フラグ、管理者トークン、dry-run成功、fixtureの排他成立フラグでは解除できない。[設定API][setupper]・[手順案の第3節](P04-VM-A-T1-runbook.md)

**REG-4　部分適用を消さない。** P・S・K・Fの各段階で、試行番号、段階・対象・予定差分、呼出し直前の意図、戻り値、読戻し確認を区別して記録する。fixtureはこの台帳と独立に「呼ばれた対象」「実際に変化した状態」を記録する。変更前から私的な記録先が使用できない場合は拒否する。呼出し後の記録失敗は効果不明に分類する。

| 結果 | 判定できる条件 | 許される次の動作 |
|---|---|---|
| 適用前拒否 | 変更API呼出し0件 | 理由と不足を返す |
| 確認済み途中状態 | メソッドが対象に及ぼした効果を独立した読戻しで特定、全段階は未完 | 実行を打ち切り、残った段階と停止理由を記録 |
| 効果不明／競合 | 変更API失敗で無作用を証明できない、応答／読戻し／台帳欠損、外部変更、対象の世代違い | 読取り専用で照合。P～Fの再送、古いリストの書戻し、サービス削除、続きからの自動適用は0件。運用では元手順のホスト判断へ |
| 登録読戻し一致 | 全効果が予定内、Vまで確認。再実行は同一試行の完了記録・所有・対象・全状態が揃うときのみ「既確認」を返せる | 再実行による書込み0件。現在値だけが似ている場合は採用しない |

APIが成功してもVの前にプロセスが終了すれば完了としない。プロセス再起動時は新たな列挙と私的な試行記録を照合し、結果不明を新規試行とみなさない。試験の強制終了は隔離された**fake子プロセスだけ**が対象であり、ゲストや本物のドライバーの障害試験は含まない。

## C　外部中止は同じ所有者で処理する

**ABORT-1　送信経路を限定する。** 将来のWindows向け制御モードでは、信頼済みのゲスト内ランチャーが試行ごとに新しい**無名・手動リセット・初期非通知イベント**を生成する。ランチャーだけが通知を発行する。子には `SYNCHRONIZE` のみの複製を `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` で明示的に継承させる。必要な標準ストリーム以外のハンドルは引き継がせず、CDOは従来どおりcollectorが非継承で開く。試行中にイベントをリセット／再利用しない。中止側にCDO、IOCTL、セッションやcall IDの発行権限を渡さない。[Windowsのイベント][event]・[権限][rights]・[子への継承][inherit]

提案する新しいCLI形式は `collect-controlled --experimental-live --duration-ms N`。Nは現行と同じ正規の10進数1～30,000。Windows AMD64と `experimental-live` の両方を必要とする。起動情報は本案では**子専用**の予約環境値 `P04_ABORT_EVENT_V1` とし、子で有効なハンドル値を固定16桁の小文字16進数（接頭辞なし）で渡す。親の一般環境をそのまま転送しない。空、0、無効・擬似ハンドル、幅超過、誤型、想定外アクセス権ならCDOのopen前に拒否する。型／権限照会に対応できない環境も拒否し、未知のオブジェクトを待機・所有・closeしない。検証して受け入れたハンドルは単一の所有者が管理し、後続の子へ継承させず、待機が戻ってから1回閉じる。受入試験では実Event型と子の付与権限を確認する。型・非通知状態・数値だけでは発行元や他ランチャーを認証できない。正規ランチャーの生成・継承経路と実物の確認は運用の前提であり、第三者が用意した有効なイベントをcollector単体で識別できるとは主張しない。権限確認にはOSが返す型とアクセス情報を使う候補を別途審査する。[オブジェクト情報][objinfo]

旧 `collect`、`audit`、`project`、default-off／非Windowsの挙動は維持する。環境値だけで新モードやlive機能を起動しない。VM-Aの試行には制御モードだけを許す案で、無効な通知経路から旧モードへの自動フォールバックはない。新たな常駐サービス・公開名のIPC・ネットワーク制御・権限昇格は追加しない。

**ABORT-2　通知と停止確認に別の時点を与える。** ランチャーの `SetEvent` 成功は「要求を発行」、ownerの待機結果が通知済みなら「要求を受理」と記録する。同じoperatorが二度通知しても一度だけラッチし、別の起動による試行には伝播させない。未通知は `WAIT_TIMEOUT`。`WAIT_FAILED` は制御経路の故障として開始前には拒否、開始後は可能な最初の境界で停止手続きへ進む。ランチャーが通知前に終了しても子のイベントは自動通知されない。独立したホスト監視は送信元喪失／到達不明を記録し、停止済みとは扱わない。[待機の戻り値][wait]

ownerは、制御経路確認後、CDOのopen前・`BEGIN` 直前、応答の検証直後、通常の`DRAIN`投入直前、空応答の待機明けに通知を調べる。待機中はイベントまたは既存の最大10 msのポーリング間隔で次の境界に進む。最後の未通知確認からAPI投入までの間に発行された場合、その境界で決まった呼出し1件は先行し得る。**ownerが受理した後の新規の通常DRAINは0件**。常にCDOは一つのFileObject、一つの発行スレッド、outstandingは最大1件である。返った呼出しは先に既存のbinding／履歴規則で処理する。最初の停止理由は一度だけ確定し、後から判明した理由は併記する。同じ境界の優先順は、輸送・応答・履歴の不確実性、通知経路故障、producer異常／予期しない状態、外部中止、収集量上限、通常期限の順。不正な応答からproducerの値を読み足さない。ホスト監視期限は別欄に記録する。[同期I/Oの仕様][dcio]・[既存の呼出し制約][builder]

| 状態・事象 | 必須の次動作 | 成果の解釈 |
|---|---|---|
| まだBEGINを投入していない | 通知済みならBEGINもSTOPも0件。既に開いたCDOは通常経路で閉じる | 新モードの提案値 `5` と固定理由 `ABORT_BEFORE_BEGIN`、stdout 0バイト。経路不正・入力不正は`2`、OS照会故障は`3`に分ける |
| BEGINを投入したが応答なし／無効 | 発行中は待つ。有効な応答が戻れば受理済みの中止へ進む。失敗・不明ならBEGIN再送も別ownerからのSTOPも行わない | 開始効果不明。現行同様、確認できないBEGINにarchiveやSTOP ACKを足さない |
| 有効BEGIN後に外部通知・制御故障・producer異常・予期しない状態 | 検証済みの返答を保存し通常DRAINを打ち切る。同じownerが新call IDでSTOP。producer fault mask非0、既知のdrop／skip非0、必要counterが未知、またはRunning外への遷移を取りこぼさない | 正常に終端まで届いても、予定した観測窓の完遂とは別。元の理由・fault・欠落は保持 |
| DRAIN失敗・応答不整合・履歴不正 | 効果不明のDRAINを再送しない。既存builderが許す新しいIDのcleanup STOPを最大1件、その後は通常／回収DRAINとも0件。通常STOPが先に失敗した場合も、新しいcleanup STOPは最大1件 | `uncertain`、call gap、元のfaultを保持。強制成功、偽終端、同一ID再送、2本目のBEGINなし |
| 正常STOPを確認、残件あり | 有効な同一ownerだけが回収DRAIN／必要なSTATUSを行う。上限・5,000 ms・既存のcleanup規則を維持。停止中の重複通知で新しい通常STOPを出さない | 停止確認と終端は別。終端は既存のACKと `Sealed / active=0 / queued=0` の条件で判定。実際の全IRP完了やdetachは未証明 |

CLIの引数・エラー出力を増やすのは新モードだけ。終了値`4`は有効なarchiveを出力した既存の意味を保つ。転送・書出しエラー、途中までのarchive、既存の2/3の区分を維持する。上記の`5`は新モードの明示中止専用で、旧CLIの意味を変えない。[既存のCLIと出力規則][cli]・[STOPと回収][loop]

**ABORT-3　記録を分離する。** native v1のfooterに新しいABORTビットを足さない。decoderは規定外のビットを拒否する。`output_committed` は従来どおり0。stdoutは従来のbinary pipeだけ。試行の私的な補助記録に、匿名試行ID、送信結果、owner受理、最初の停止理由と併発理由、BEGIN／STOP ACK、終端、監視期限、プロセス終了、受信側のバイト数・ハッシュと保存結果を分けて残す。観測していない状態はunknown。ownerはCDOの呼出し経路で同期的な診断書出しを待たず、確定した理由を固定サイズで保持し終了時に別の診断経路へ返す。親は標準出力・診断を並行して受け、停止要求の発行と監視結果を独立に記録する。記録が失われた場合は補完しない。有効終端の後に初めて届いた通知は「終了後の要求」として外側にだけ記録し、再open・追加STOP・archive書換えは行わない。[footerの許容値][format]・[既存の終端判定][terminal]

**ABORT-4　同期I/O不帰還。** `CreateFileW`、`BEGIN/DRAIN/STOP/STATUS` の同期交換、stdoutのwrite/flushが戻らない場合を別々に扱う。5,000 msは応答が戻った後のループ境界で確認するpost-STOP値である。試行の外の監視は前案どおり、初回1,000 ms窓の例で**プロセス起動から15,000 ms**を判定点とする。これは停止期限の保証ではない。期限時点で未終了なら、確定済みの事実だけを保存し、新規試行を打ち切る。ゲスト内の同じ障害に巻き込まれないホスト監視が運用時に必要。kill、別ハンドルSTOP、CancelIoEx、強制close、強制電源断、自動R1復元を中止通知の代用にしない。出力停止ではSTOPが成立している場合もあり、プロセス終了・証跡保存を独立に判定する。[既存の5秒試験][deadline]・[Windowsの取消制約][cancel]

## B　将来の受入単位と引き渡し

登録のモデル、Windows登録境界、中止通知を別々に判定する。双方が必要なVM-Aでの試行には片方の合格を流用しない。具体的な実施順、固定ケースとoracle、ビルド証跡の受入は[試験条件](P04-T1-cancel-test-matrix.md)に記載した。ソース／ビルドの合格には実行した証跡が必要である。文書レビューでは要求と試験可能性だけを判定する。

運用への引渡しでは、未変更なら中止を記録し、部分適用・効果不明なら新規試行を止めてR1外へ得られた証跡を退避する。正常終了を確認できた場合に、別途許可されたホスト担当がR1だけを選んで復元し、OSとT1を再同定する。ゲストが応答しない場合は停止未確認のまま所有者へ引き渡す。本検証の試験装置はこの判定・記録までを扱い、VMwareや電源の操作を呼ばない。

実機固有の排他、パッケージの配備・署名・HVCI、実際の装着順序（T1の `partmgr → P04 → disk → stornvme` は候補）、CDOの実効ACL、実停止／ホストからの回復は後続ゲート。R1への正常停止後の復元実績はあるが、ハング中の強制復旧許可はない。実行の可否は前の手順案に戻り、別途所有者と判定する。

[selector]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/wdk/driver.c#L21-L125
[attach]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/wdk/driver.c#L143-L183
[cli]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/main.rs#L28-L107
[win32]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/win32.rs#L74-L132
[owner]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/contracts/upper-candidate-probe-v1.md#L84-L98
[open]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendeviceinfow
[upper]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-upperfilters
[multi]: https://learn.microsoft.com/en-us/windows/win32/sysinfo/registry-value-types
[extension]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/using-an-extension-inf-file
[setupper]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw
[event]: https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createeventw
[rights]: https://learn.microsoft.com/en-us/windows/win32/sync/synchronization-object-security-and-access-rights
[inherit]: https://learn.microsoft.com/en-us/windows/win32/procthread/inheritance
[objinfo]: https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntqueryobject
[wait]: https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject
[dcio]: https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-deviceiocontrol
[builder]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-core/src/archive.rs#L373-L468
[loop]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/capture.rs#L98-L170
[format]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-core/src/archive.rs#L108-L122
[terminal]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-core/src/archive.rs#L492-L534
[deadline]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/capture.rs#L329-L355
[cancel]: https://learn.microsoft.com/en-us/windows/win32/fileio/canceling-pending-i-o-operations
