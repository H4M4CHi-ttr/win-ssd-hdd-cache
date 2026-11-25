# P04　元のT1に限るデバイスインスタンス登録契約案

2026年11月23日。対象は実験用Windows VM-Aの元の8 GiB VMDK「T1」。D01は同じVMのOSディスクを指す。登録前の照合をPRE、装着後の照合をPOSTと呼ぶ。

**設計上の選択は、ゲスト共通のサービス配備と、T1のデバイスインスタンスへの登録を分けること。実登録はNO-GO。** 配備するprimitiveパッケージにはModelsを持たせず、登録担当がT1を個体として指定する。書き込んでよいのは、T1に未作成の個体別 `UpperFilters` をP04サービス名1件で新設する操作だけである。既存の値を編集する分岐は設けない。Windowsには個体を指定するsetterがあるが、対象の世代と「まだ未作成」を同時に保証して書き込める裏付けが今はない。下記の条件がそろうまで、最初のパッケージ配置も発行しない。[個体別UpperFilters][upper]／[指定個体のプロパティ更新][set]

これは11月20日の[拡張INFによる契約案](P04-T1-registration-restart-first-load-contract.md)から、対象選択と登録の方式を変更する案である。[リスト全体を更新する旧方式のNO-GO](P04-T1-registration-exclusion-and-package-scope.md)は引き継ぐ。文書の受入、実装の受入、VM-Aへの実登録許可はそれぞれ独立した判断である。

## 根拠に使ったVM-Aの記録

11月22日のホストとペアリング済みゲストの記録による。D01は全行で `NVMe 0:0`、PCIバス19の親、LUN 0。比較はいずれも接続中の2台について、Hardware IDsとCompatible IDsを別々に取得した結果である。

| 時点（JST） | 同じT1の接続とゲストでの同定 | D01とT1のINF照合 |
|---|---|---|
| 22:28、往路前 | `NVMe 0:1`、D01と同じ親のLUN 1 | Hardware 8件、Compatible 2件が共通。T1固有0件 |
| 23:23、往路 | `NVMe 1:1`、別の親（PCIバス27）のLUN 1。旧位置に残った非接続T1とも照合 | 共通10件、両配列の順序も一致。旧T1とのID差分も0件 |
| 23:41、復路 | `NVMe 0:1`、元の親のLUN 1。両ディスクとも開始済み、問題コード0 | 共通10件、T1固有0件。両台の有効順序は `partmgr → disk → stornvme` |

ホストではR0を親とするR1と、D01・T1の現在のVMDKから元のファイルまでの連鎖を往復前後に照合した。復路後には子が0台の第2NVMeコントローラーが残り、ゲストでも `present/started` だった。今回の設計はこのコントローラーを削除せず、元のT1を対象とする。ゲストbuild `26300.9974` と上表は過去の測定値である。8 GiB・RAW・区画0・特殊用途不使用の一式は以前の `AFTER_R1` の記録にある。直近の通常権限のPREではOS・EFI区画やRAW・区画数を再確認できなかった。ボリュームの関連件数で不足分を埋めない。

## D01を保護する範囲と、T1を指す証拠

| 対象 | 将来の1試行で許す効果 |
|---|---|
| T1の個体 | 個体別上位フィルターの新設1件、T1だけのデバイス再始動1回、P04の初回装着1個、読み取りによる確認。初期化・フォーマット・意図的なストレージI/Oは対象外 |
| D01、他のデバイスノード（devnode、非接続の個体を含む）、親・兄弟、クラス | P04参照の追加、配備に伴うデバイス再構成・再始動、P04の装着・観測はすべて0件。対象外への変更要求も0件。既存の上下フィルター、機能ドライバー、volume・dump構成を保つ |
| ゲスト共通 | 固定した1パッケージのDriver Store・カタログ管理、専用の要求時起動サービス1件、P04イメージの初回ロードと所定の制御デバイス（CDO）だけを将来の承認票に列記する。これらはOSを置くD01上のファイル・レジストリに通常の書き込みを生む。この書き込みまでゼロが要件なら開始不能 |
| ホスト | ゲスト外で試行を監視し、承認された条件下でだけR1へ戻す。R0/R1の削除・統合、VMDKの変更、コントローラーの後始末は別作業 |

登録前に、ホストでVM-A・R0/R1・両VMDKの連鎖と現在のノードを確認する。ゲストでは、現在のストレージ固有ID・シリアル、完全なPnPインスタンスID、親の位置とLUN、OS・EFI/boot・pagefile・dump・休止の所在を私的に結び付ける。D01と元のT1は各1台、T1は8 GiB・RAW・区画0・特殊用途0を要する。最初の配置前に未取得、同一IDの別実体、曖昧な旧devnode、入れ替え、保護用途の不明が一つでもあれば、変更要求0件で終了する。配置後に失効した場合は、既に発行した配備・登録と未発行の後続段階を別々に記録し、回復へ進む。ディスク番号、ドライブ文字、スロット、モデルIDだけで書き込み先を選ばない。[デバイスインスタンスID][instance]

D01について保護するのは、試行の配備・登録・再始動が起こすデバイス作用である。既存状態を確認する読み取り、通常のOSのバックグラウンドI/O、別に承認する復旧時の正常なゲスト起動は記録上も分ける。D01の再構成を検出して試行との関係を区別できない場合は不合格とする。

ここでいう「世代」は、同じT1がその時点でそのdevnodeに結び付いている期間を指す。試行番号や自作カウンターをWindowsの世代保証には使わない。正常な再始動でも物理デバイスオブジェクト（PDO）が変わり得るため、登録時の世代と開始後の世代を別に記録する。後者を受け入れるのは、承認した1回の再始動と元のVMDKに独立に対応づけられた場合だけである。

## 配備と初回ロードを分けて設計する

**対象を選ばない配備物。** `Manufacturer/Models` のないAMD64用primitiveパッケージを候補とする。INFの実効節と参照先をすべて展開し、専用イメージとサービス以外への作用を拒否する。付随INF、クラスへの登録、機能ドライバーの交換、共通SYSの更新、サービスの自動開始・上書き・トリガーを許さない。現在および再登場し得る機器・クラス・サービス設定にP04を参照する残存箇所がないことも必要である。primitiveの `DefaultInstall` はデバイス登録には使わない。1903以降のprimitive向け `DiInstallDriver` はパッケージとサービスの処理を伴い得るので、最終配布物について「配置だけ」の副作用を仮定しない。[primitiveの要件][primitive]／[DefaultInstallの用途][default]／[Driver Store][store]

最終CATの信頼とINF・SYSを含む全メンバー、サービスの起動種別・イメージパス・実ロード用イメージのハッシュを固定する。検査から使用まで差し替えられず、専用名を他のインストーラーが先取りしても既存サービスを更新しない根拠が要る。新しいパッケージは対象ゲストの署名方針とHVCIの下で受理・ロード可能であることを別途検証する。Secure Boot、HVCI、テスト署名の変更を前提にしない。[カタログの検証][signature]／[カーネル署名方針][signpolicy]

**初回ロード前に対象を固定する。** この案では、試行番号と、T1の完全なインスタンスIDから求めた照合用ハッシュを、将来署名する実験用SYSの読み取り専用データに含める。照合入力は固定のASCIIタグ `P04/T1/instance/v1`、試行ごとに一度発行する32バイトの乱数（nonce）、終端NULを除くUTF-16コード単位数（4バイト、リトルエンディアン）、その全単位のUTF-16LE表現の順に連結し、SHA-256を計算する。入力の型・終端・長さを先に検査し、埋込みNUL、切詰め、欠落を拒否する。大文字小文字も保持するため、表記が変われば同じ機器でも装着を拒否する。ホストの私的な承認票に元の文字列とVMDKの照合結果を保持し、署名後にINF・CAT・SYS・登録担当プログラムの最終ハッシュと試行番号を対応づける。ハッシュ一致が示すのはインスタンスIDの一致までである。VMDKの同一性とdevnodeの世代は別に証明する。試行ごとの署名と秘密情報の取扱いには別承認が要る。[署名とカタログメンバーの検証][signature]

新しいP04は最初からこの対象にしか装着できない仕様とし、レジストリから後付けするselectorへのフォールバックを設けない。完全一致はドライバー側の二重確認である。D01がインストール対象に選ばれた後に `AddDevice` が装着を見送っても不合格である。ホストの試行記録は再送・R1復元後の再利用を拒否する。ドライバーのアンロード・再ロードをまたぐ一回性は、nonceやメモリ内フラグだけでは保証できない。

**既存コードとの差。** 受入済みツリー `f1fb5c4f9f459edd2aeebfc490bef246f4a2f7d7` の `driver.c` 72・98行では、`DriverEntry` が可変の `TargetInstanceId` を一度読み、`AddDevice` は不一致でも未装着のまま成功し得る。143–157行では制御デバイスとリンクを実attachの前に作る。`boundary.rs` 242行の既存登録は全リストの比較・設定・再読戻しで、`real_apply_gate` は `RealApplyUnavailable` を返す。この案のselector、条件付き登録、起動制御を実装済みには数えない。照合した配布ZIPには検証用INFが1件、CAT・SYSは各0件だった。[審査済みソース一式](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/tree/153fac3473186241ccf3f354e59322e63b1f183f)／[既存ソースの受入範囲](P04-source-build-review.md)／[上流のP04証拠](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/evidence/P04.md)

## 他のインストーラーとPnPが動いても、他者の登録を失わない条件

契約が要求する**条件付き登録**を次のように定義する。これはWindowsのAPI名ではない。呼出しの宛先は、完全なインスタンスIDで開き、元のT1に再照合した現在のデバイス情報要素だけである。試行担当はD01やクラスを変更対象として開く経路を持たない。[指定個体を開くSetupAPI][open]

| 項目 | 受入条件 |
|---|---|
| 書き込み前の状態 | 同一世代のT1が一意にpresentで、用途・子・再始動の影響範囲が承認時と一致し、個体別 `DEVPKEY_Device_UpperFilters` が**未作成**。存在する空リスト、既存名1件以上、重複、破損、権限不足、不明はすべて適用外。クラスの `partmgr`、`EhStorClass` や個体別下位フィルターには触れない |
| 1回の変更 | 上の同一性・世代・未作成条件が書き込み成立時にも真であるときだけ、個体別プロパティ（`DEVPROP_TYPE_STRING_LIST`、従来形式は `REG_MULTI_SZ`）に専用サービス名1件を新設する。判定後・新設前に他者の書き込みや対象の入替えが成立し得る方式は採用しない。既存値へのappend・置換・削除の経路はない |
| 結果 | **変更なしの競合**、**確認済みの新設**、**効果不明**を区別する。競合で0件とするには不成立・無変更の積極的な証拠が要る。応答喪失、プロセス終了、書き込み後の読戻し失敗は効果不明。拒否が確定しても、同じ試行を再送しない |
| 効力を保つ期間 | パッケージ・専用サービス名・固定イメージは最初の配置前からR1復元確認まで保護する。T1の対象世代・既存フィルター・開始経路は新設直前から再始動・POSTまで保護する。T1の用途と影響範囲の保護は配置前に取得し、POST後も正常停止まで維持する。遅延登録・余分な再始動を防ぎ、復元まで新試行を発行しない |

**用途を変える書き手も含める。** 保護の強制主体は、将来の実装で根拠を示すWindows側の登録・ストレージ調停機構とする。ホスト監視だけにこの役割を負わせない。T1はRAW・区画0・特殊用途なしを保ち、子、利用中のvolume、除去・再始動の依存先は承認範囲内でなければならない。ディスクを初期化する管理処理やページファイル等の割当処理は、devnodeを取り替えずにこれらを変え得る。新設が成立する時点と、次節の再始動を取り消せなくなる時点の**両方**で、条件の再確認と変更防止を一体にする。どの書き手・PnP経路が拘束されるかを示せない場合は、最初の配置を許さない。ディスクのロック・オフライン化をこの機構の代用にしない。

実装候補は、(a) Windowsが同一個体・同一世代について条件判定と新設を不可分に扱うことが文書化された経路、または (b) 影響する**全書き手とPnP**が拘束される区間内で、指定個体のsetterを使う経路である。どちらも、既に旧値を読んだ書き手の遅延書込み、新たなインスタンスへのID再利用、登録後の再列挙・サービス再利用まで扱うことを要する。無関係な作業は継続できる。信頼するのはホストの監視担当、対象Windowsのカーネル、固定したパッケージと登録担当である。通常の別のインストーラーやWindowsのPnPが同じロックに従うとは仮定しない。第三者がD01に同サービスを結び付けられる期間が残るなら開始しない。

候補(a)の保護は新設の一瞬に限られる。以後の他者による遅延上書き、対象の付け替え、余分な起動を防ぐ仕組みを別に示す。候補(b)では保護の取得・失効・解放を誰が強制するか、既に進行中の書き込みも含めて証明する。他の書き手に対してこれらの条件を守れない場合は、どちらも使えない。検出してR1へ戻すだけでは、すでに失われた他者の登録は保護できない。

**現時点で、いずれのWindows経路も条件を満たすとは認定しない。** `SetupDiOpenDeviceInfoW` は宛先を指定できる。`SetupDiSetDeviceRegistryPropertyW` は既存値・世代を期待値として受け取らない。個体が消えて同じ名前で戻る場合への保証もハンドル保持だけでは得られない。`CM_WaitNoPendingInstallEvents`、通知、独自mutex、管理者権限はこの区間を保護しない。たとえば他者が先に値を読み、こちらの成功読戻し後に古い内容を書き戻せば、途中までの記録は正常でも登録は失われる。[setterの引数][set]／[待機APIの保証範囲][wait]／[PnP通知][notify]

`AddReg` の `NOCLOBBER` は既存値を保持し、`APPEND` は複数文字列値を扱う。どちらもdevnode世代の条件や再始動までの他の書き手の排除を約束しない。`SetupInstallFromInfSection` の一般的なレジストリ処理を、Modelsから独立した `DDInstall.Filters` の適用だとは扱わない。`DiInstallDevice` で1台にパッケージを選んでも、同じパッケージの将来の他機器への適用を封じる証拠は別に必要になる。オフラインの `Enum` 編集、クラスフィルター、基本INF差し替え、D01にインストール制限をかける方式も本契約の対象外である。[AddReg][addreg]／[DDInstall.Filtersの必須参照][ddfilters]／[1個体へのパッケージ指定][diinstall]／[PnPレジストリの扱い][regsafe]

## 打切り後に新しい要求を発行させない

ホストの監視担当とゲストの登録担当は、各段階の **意図 → 取消可能な発行許可 → 取消不能な引渡し → Windowsの受理 → 効果確認** を区別する。引渡しとは、停止と発行を調停する実行主体が、特定の試行・世代・段階・宛先の要求そのものを不可分に引き受ける時点を指す。以後、ゲストのワーカーが新たに判断してWindows呼出しを発行する余地を残さない。普通のワーカーに許可を配る時点、期限確認、台帳への「送信済み」記入は、この境界を証明しない。

各段階は直前段階の効果確認後に一つだけ許可する。許可の失効・期限切れ・接続断と取消不能な引渡しに一つの順序を付け、失効が先なら後から再開したワーカーにも発行させない。引渡しが先なら、当該1件だけは期限後にもWindowsに受理・完了され得る。順序が判別できなければ「この1件は既に発行され得る」、次段階は未許可として停止する。再接続・プロセス再起動時に保留の許可を再利用しない。ホストのタイムアウトは失効の要求時刻であり、強制主体が受理した時刻やWindowsが取り消したこととは別に記録する。

この順序を保証できる実行境界が実登録の前提である。タイマー、heartbeat、ホストでのnonce消費、ゲストでの「期限確認後にAPIを呼ぶ」だけでは、確認と呼出しの間に停止していたワーカーを遮断できない。分断時に新規要求を拒否できる証拠がなければ配備も始めない。

## 再始動はT1の1回に限り、位置は実オブジェクトで確かめる

1. **発行前。** T1の基本INF・節・版、個体別・クラス別・宣言的な上下列、現在の有効スタックを区別して採取する。過去の両台は `disk.inf` の `[disk_install.NT]` で、`partmgr` はDiskDriveクラスの上位、`EhStorClass` は同クラスの下位登録だった。当時は両台の個体別Upper/Lowerの値がなく、宣言的な合成列は未掲示だった。基本INFの有効節と参照先に上位レベル定義は確認されていない。クラス登録と個体別登録をこのビルドでどう併合するか、目的の位置になる根拠を審査する。位置を説明できない、同一レベル内の並びに依存する、POSTの証拠手段がない場合は開始しない。[フィルター順序][order]／[ADR-004](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/adr/0004-interception-boundary.md)
2. **準備・登録・起動。** ホストはR1外の台帳に、試行番号、各段階の宛先・最大回数・有効期限を先に記す。配備物とサービスを確認し、T1の条件付き登録を1回だけ発行する。初版は、登録自体が余分な開始を誘発しないことを示し、同じ試行の保護下でT1の再始動要求を明示的に1回だけ発行する方式に限る。再始動と初回ロードに別々の人手を挟まない。暗黙の再始動・先行ロード・結果不明があれば、手動再始動を重ねない。署名済みSYSの初回ロードはゲスト単位で記録する。
3. **再始動の範囲。** 机上の評価対象は、T1だけのデバイス情報要素に対する `DIF_PROPERTYCHANGE`・`DICS_PROPCHANGE` とする。参加するクラスインストーラー・共同インストーラー、既定処理、返される `DI_NEEDREBOOT/DI_NEEDRESTART` を審査する。OS再起動への切替は許可しない。T1の子、`RemovalRelations`、共有する機能／フィルタードライバーやファイル更新による波及先を含めて影響範囲がT1のみと証明する。子ボリューム0件はその一部である。`SetupDiRestartDevices` は条件により同じドライバーを使う他機器も再始動し得る。D01・親・兄弟に及ぶ経路、OS再起動要求、範囲不明の経路は発行しない。[プロパティ変更時の停止・再始動][propchange]／[DIFハンドラーと再起動フラグ][classinstaller]／[再始動APIの適用範囲][restart]／[RemovalRelations][relations]
4. **POST。** 同じ試行・世代遷移で再同定したPDOに対応する実デバイスオブジェクト列を、事前に審査した既設の読み取り手段で採る。T1は上から **`partmgr → P04 → disk → stornvme`** でP04が正確に1個。D01は **`partmgr → disk → stornvme`** でP04が0個。両隣の実オブジェクト、イメージの帰属・初回ロード時刻、T1の正常な再開、保護対象の不変を照合する。`PnPUtil /stack` の有効構成、サービスRunning、CDOの存在、`AddDevice` の成功値だけでは合格にしない。[PnPUtil][pnp]／[デバイスオブジェクト列][devstack]
5. **位置と観測を混同しない。** `IoAttachDeviceToDeviceStackSafe` は呼出し時の最上位に装着し、実際の下隣へのポインターをI/Oのロック下で設定する。稼働中の完成済みスタックの中間には挿入できない。直前の最上位照会とattachの間、さらに上側が組み上がるまでの順序はこのAPIだけでは固定できない。誤装着を事後に見つけた時点で失敗とし、一時的な誤装着を合格へ繰り上げない。[attachの仕様][attach]
6. **制御デバイス。** 予定した名前・名前空間・イメージ・実効ACLを固定する。既定のSDDLがSYSTEMと管理者に限定されても、設定による上書き、既存名との衝突、公開タイミングを審査する。新設計ではT1の装着成立前と装着失敗時に、利用可能な入口を0件とする。成立前に、利用者から参照できるNT名付きCDO・シンボリックリンク・デバイスインターフェイスは作らない。外部の `IRP_MJ_CREATE` の成功は0件とし、旧試行の生存ハンドル・同名オブジェクトが1件でもある場合、または不存在を確かめられない場合は配備前に止める。成功経路ではT1のattachと下隣の確認後にだけ、固定名のCDOとリンクを所定のACLで作る。この登録確認専用の試行中は、NT名・リンク経由のCREATEを受け付けない。既存・継承・複製ハンドル経由を含めて `BEGIN` 成功と観測レコードは常に0件。リンクの削除だけで既存ハンドルが失効したとは扱わない。生成・公開・ACL検証が実attach後に失敗すれば部分成功である。受付を閉じ、既発行IRP・FileObjectのrundownとPnP除去の順序を守る。試行が所有する名前と参照だけを解放し、残存参照を検証する。未装着時の削除処理の流用、強制close、即時アンロードは禁止する。collector、CDOを開く試験、IOCTL、ダンプ・デバッガの新規設定は承認対象に含めない。既設手段で実効ACL・実装着を確認できなければ未確認として回復へ進む。[名前付きデバイスオブジェクト][named]／[制御デバイスのセキュリティ][cdo]／[既存の停止契約](P04-T1-cancel-spec.md)

生の識別値・パス・オブジェクト対応は私的証跡に限り、共有する結果は匿名化する。ホストは、発行前の意図、ゲストが発行した対象と要求、APIの返答、対象世代に対応するPnP開始・終了と実イメージロード、POSTのオブジェクト列を試行番号に結び付けて受領する。D01・他個体・クラスの前後差および期間中の変更を、登録担当とは独立した、範囲と欠落を判定できる証跡で確認する。前後の値が同じでも途中の再始動やABAは否定できない。時系列の欠損を0件にせず、必要な既設の証拠経路がなければ発行しない。

## 異常時の停止とR1への回復

**将来の承認は未発行。** 所有者が「VM-Aの特定パッケージの配備・T1への登録・T1の再始動・初回ロード・POSTを1試行」と、「通常停止・ホストからのR1復元・検証起動」を別欄で明示して初めて対象にする。ホスト担当、保持するR0、戻すR1、両VMDKの連鎖と空き、R1以降の変更を失う範囲、ホストの時計で測る適用・応答・正常終了の各上限、例外時の引渡し先を先に固定する。R1へ戻すと現在の第2コントローラーやWindows側の状態も変わり得る。現在の状態の保存が必要なのに退避・破棄条件を合意できない場合は開始しない。[既存のホスト復旧手順](P04-VM-A-T1-runbook.md)／[スナップショット復元の効果][revert]

| 発生時点 | 処置と証拠 |
|---|---|
| 最初の変更前 | 対象・方式・署名・排他・位置・復旧の根拠に欠落があれば中止。変更要求0件を立証できれば「未発行」。別の作業の痕跡を消さない |
| 配備後、登録中、再始動中、POST後 | 競合、対象の消失、誤った世代や位置、対象外要求、余分なロード、応答喪失・期限超過を検出したら新規の変更を止める。発行・応答・確認した効果・不明を区別して試行を廃止する。自動再試行、古いリストの書き戻し、個別アンインストールによる修復をしない。待機の打切りでカーネル処理が取消済みとは扱わない |
| ゲストが応答し、ホストで正常終了・非サスペンドを確認 | 成功した初回試行も、部分成功・効果不明の試行も、事前承認があればホストが指定R1のみへ戻す。R1外に匿名の時系列・差分・欠損を残し、退避を無期限に待たない。VM-A、親R0・R1、OS・T1の連鎖と復元権限に疑義があれば発行しない |
| ハング、BSOD、停止不明、監視・復元の失敗 | ゲストの応答停止や稼働一覧からの消失だけで正常停止を宣言しない。「停止不明／復元未確認」としてホスト担当から所有者へ引き渡す。強制電源断・稼働中復元・R0への切替・ディスク差分の手編集は別承認。例外時の扱いを事前に決められない試行は開始しない |
| R1復元後 | ホストがR0保持・R1の位置、両VMDKの接続・親子を確認して通常起動。ゲストで両台を再同定し、元の用途、対象／他個体・クラスのフィルター、サービス・パッケージ・ロード済みイメージ・残存参照をR1の基準と照合する。アクセス不足や差の原因不明なら「復旧未確認」。R1への復元成功は、試行中にD01へ作用しなかった証拠には使わない |

## この案を実登録の審査に戻す条件

以下は将来のソース／合成試験で反証する条件であり、今回は試験を実行していない。各失敗で、許可対象外への要求0件、打切り後の新しい引渡し0件、結果不明を成功にしないことを検査する。既に取り消せなくなった1件は別に集計する。書き込み直前の拒否と、書き込み後に判明した失敗は別の期待結果にする。

| 反例・試験入力 | 合格に必要な挙動 |
|---|---|
| D01を選んだ、名前や番号だけが一致、旧T1と現T1が混在、同じIDで別VMDKが再登場 | D01・旧個体への登録／再始動0。承認時の世代を証明できず中止し、別個体へ探索し直して適用しない |
| 未作成と空値・破損・読取拒否の混同、既存の個体フィルターあり | 真に未作成の場合だけ1回の新設を許す。その他は他者の値をバイト・順序ごと保持し、設定要求0件 |
| PRE後に他者が値を作成、旧値での遅延後書き、変更後に元へ戻るABA、devnode消失・再列挙 | 条件が先に失効したら新設0件。新設後の干渉を防げる根拠がなく、あるいは遅延操作の帰属が不明なら実バックエンドを採用しない。衝突注入で効果を観測した試行は即停止・回復対象 |
| 配備後に同じインスタンスのT1を初期化する競合、新設後・再始動前に区画や用途・停止依存先を変える競合 | 新設前の失効なら配備済み・新設0・再始動0。新設後で再始動が未発行なら配備済み・新設1・再始動0。いずれも回復対象。保護中の用途変更を防止できない実機経路は採用しない |
| 発行許可後・取消不能な引渡し前の切断、ワーカー中断・期限切れ後の復帰。逆順では引渡し直後に受領記録を喪失 | 失効が先ならWindowsへの要求0件。引渡しが先なら当該1件まで、境界不明なら効果不明。後続段階・再発行・再接続による再開はすべて0件 |
| サービス名の同時取得、古い試行票、別ハッシュ、初回ロードの先行、D01での `AddDevice` | 誤配備・誤装着を許さない。D01を登録した実装はselectorで拒否しても不合格。副作用を発行済みなら再送しない |
| 再始動が親・兄弟に波及、二重再始動、想定外の上下関係、CDOだけ出現、ACL拡大、POST証跡欠落 | 装着確認0、観測開始0。実オブジェクト列・D01不変・T1正常再開をすべて立証した単発試行だけを装着確認とする |
| 正しい／異なる完全ID、表記だけ違うID、長さ不正。NT名だけ見える、リンクだけが消える、旧ハンドルが残る、attach後にCDO公開が失敗 | 規定バイト列の試行票だけを受理。早すぎるNT名公開・CREATE成功、いずれのハンドルからのBEGIN成功も不合格。attach後の失敗は新設済みとして、通常のPnP寿命とホスト回復で処理する |
| 各変更の発行直後に登録担当が終了、同期呼出し不帰還、監視または正常終了不能、復元後の不一致 | ホストが期限と最後の確定事実を保持し、二重発行0。正常終了を確認できる場合だけ承認済みR1へ進み、復元後照合の失敗を明示する |

最初に必要な成果物は、このWindowsで使える**条件付き新設、用途・世代の保護、失効を強制できる単発再始動**を、実在するサポート対象の手段とその保証区間へ対応付けた根拠である。固定した署名済み配布物、現在の用途確認、位置の根拠と実装着の証拠経路、実効ACL、R1への回復許可も個別に必要になる。合成試験の成功だけでは実登録を開放しない。条件を満たす実経路を示せなければ、本案の実適用は棄却する。

[upper]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-upperfilters
[set]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw
[instance]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/device-instance-ids
[primitive]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/creating-a-primitive-driver
[default]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-defaultinstall-section#remarks
[store]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/driver-store
[signature]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/verifying-the-release-signature
[open]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendeviceinfow
[wait]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_waitnopendinginstallevents
[notify]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_register_notification
[addreg]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-addreg-directive
[ddfilters]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-ddinstall-filters-section
[diinstall]: https://learn.microsoft.com/en-us/windows/win32/api/newdev/nf-newdev-diinstalldevice
[regsafe]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/accessing-registry-keys-safely
[order]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/device-filter-driver-ordering
[restart]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdirestartdevices
[relations]: https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/irp-mn-query-device-relations
[pnp]: https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/pnputil-command-syntax
[devstack]: https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/-devstack
[attach]: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddk/nf-ntddk-ioattachdevicetodevicestacksafe
[named]: https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/named-device-objects
[cdo]: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdmsec/nf-wdmsec-wdmlibiocreatedevicesecure
[signpolicy]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-policy--windows-vista-and-later-
[propchange]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/ns-setupapi-sp_propchange_params
[classinstaller]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdicallclassinstaller
[revert]: https://knowledge.broadcom.com/external/article/315595
