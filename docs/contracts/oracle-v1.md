# Portable independent oracle v1 — D07

状態: specificationは独立Astra review受入済み（`cefd96ea936e664c648ba67437a233261a58a92b`）、PR #71でmain統合済み（2026-11-03）。[D02 trace-v1](trace-v1.md) の意味を変更せず、F06/F07が使う独立判定と証拠を定義する。[受入範囲](../adr/0017-test-oracle.md)を参照。Windows/WDK/IRP/MDL/DMAの実証ではない。candidateが返すgeneration、valid、successをそのまま正解として扱わない。

## 1. 分離する責務

| component | 入力と責務 | 禁止する近道 |
|---|---|---|
| F05 parser | 型/上限/static参照を検査したTraceV1 | stale/phase違反をparserで排除してmutant検出に数える |
| deterministic scheduler | 配列順を固定し、candidateへの操作と下位eventを駆動する | hidden RNG、wall clock、thread順、勝手なfallback挿入 |
| fake lower storage | 原本/SSDの実際のsample/apply/complete/persistを記録する | traceから成功bytesを直接callerへ注入する |
| candidate/reference policy | D02 decision、metadata、所有権のclaim、callerへのresult | oracleの判定関数を呼んで自身を正しくする |
| independent oracle | 下位履歴と独立ledgerからcandidateのclaimを検査する | candidateのgeneration/valid bitやexpected assertを正解として使用する |
| trace assertions | 手で固定した期待結果との一致 | runnerの出力から期待値を生成する |

Rust moduleは少なくとも`replay.rs`（driver/reference）と`oracle.rs`（独立ledger/判定）を分ける。型・checked range・固定error vocabularyは共有可。freshness/lease/publication判定の共通helper、candidate状態のcloneをoracle状態にする実装は禁止。candidateに故障を入れてもoracleとfake lower storageは変化しない。F04 MemoryOrigin/CompletionQueueは小さい部品であり、durable/version/owned lineage ledgerの代わりではない。

## 2. 信頼できる観測とclaim

下位観測はharnessが実際に実行した操作から作る。candidateが要求したdescriptorはoracleにも渡し、要求範囲/source/親との対応を検査してからlowerへ送る。以下はpublic wire eventではなく、F06内部の型付きrecordである。

- Parent admission record: event index、request、kind/range、admittedか、write payload/FUA、flush frontier。
- Child forwarding record: event index、io/parent/target、immutable range/source/slot、実際にforwardしたか。pending/rejected submitは実I/Oを作らない。
- Transfer record: event index、io、child-relative fragment、lowerから得た値/known mask、mutation apply-version、slotへ実copyされたsource lineage。
- Actual completion record: io、lower status/reported、実終了index。一度だけ。cancelは別flagでありcompletionではない。
- Candidate claim: bind成功、cache write/read admission、publish成功、caller commit（source io、bytes/mask/status/count）、retire/resume。各claimの直前/直後indexを固定する。
- Persist / session-end record: 実際にdurableへ適用したmutation fragment/version、power-loss前のpending集合。

oracleはcandidateに「自分はwriteを完了した」と言われただけでactual ledgerを進めない。下位io_completeにまだ到達していなければearly ACKを検出できる必要がある。candidateがclaimを出さないことは観測成功ではない。安全検査とtraceのdeterministic decision検査の両方で、何もしない実装を全面合格にしない。

## 3. 独立原本履歴

各byteのvolatile/durable cellは`(value, known, apply_version)`。initialはknown、version 0。unknownは値をzero placeholderへ正規化するが、known=falseを失わない。各mutationの非zero transfer fragmentに単調なapply-versionを与える。traceの10,000event上限内のevent index由来でもよいが、counter_limit（cache generationの反例設定）と混同しない。

1. origin_writeはpayloadから該当fragmentをvolatileへ適用する。
2. origin_discardはfragmentをunknownにする。非転送位置は変化しない。
3. request/ioごとに実際のapplied cells/versionsを保持する。reported count、最終status、retireで適用履歴を消さない。
4. origin_readはfragmentごとの**その瞬間**のvolatile cellsをsampleする。後でoriginが変化してもsampleを更新しない。
5. origin_persistのoffsetはchild-relative。実際に適用済みのcellsだけを対象とし、byteのdurable versionがより新しければ旧persistはno-op。coverageのない位置をpersistする入力はsemantic error。
6. power_lossはvolatileをdurableへ戻すwhole-session終端。cacheを捨て、未完了をabandonedとして別計上する。再起動後のcache reuse、現実のDMA解放の許可ではない。

非zero transferは同一child内で重複不可。zero lengthは範囲内のno-op。F06がoverlap/complete後transferをsemantic errorとすることと、candidateが未完了I/Oを解放した安全違反は分ける。

## 4. 許容read / result

### 原本read

caller commitは実終了済みの対応origin_readから一度だけ行う。lower status/countを変更せず、reported prefixのsample cells/known maskを返す。そのprefixはsample済みでなければならない。全rangeを一度の時点のsnapshotであると仮定しない。

例: originが`[A,A]`、先にbyte0をsample、その後writeで`[B,B]`、次にbyte1をsampleすると、`[A,B]`は許容される。commit時の現在origin`[B,B]`と比較して誤ってFAILにしてはいけない。逆に、byte0もwrite後にsampleしたのにAを返すのは不一致。

### キャッシュread / publish

oracleは次の独立履歴を使う。candidateのgeneration値は入力しない。

- 各fillのadmission indexとsource readのsample履歴。
- admit済みのmutationの**要求範囲**、admission index、actual origin completion index。未完了writer数はこの集合から導出し、candidateのcountを信用しない。
- 全slot bind、全cache_write admissionとphysical fragmentsのordered history、actualに残るread/write lease集合、直近のcontent owner IO。
- epoch invalidation/resumeのordered history。invalidationは物理leaseを消さない。

publish claimが許されるのは、同一fillに由来するfull-known/full-coverageのsourceとfull successful cache writeが実在し、そのfill admission以後に交差mutation admissionまたはepoch invalidationがなく、現在交差writerがなく、slotの直近bind/content ownerがこのchildで、未終了write leaseがない場合だけ。古いwriteを失敗させても前のvalid lineageを復活させない。

cache_read admissionとcommitの両方で、対応する許容publish lineage・要求range・full known coverage・slotのphysical ownerを確認する。読みleaseのadmission後に交差mutation/epoch/slot変更があればcommitは許されない。actual read完了でleaseが消えても、commitまでの失効を見落とさない。commit後のimmutable caller resultは後発writeで変更しない。

oracleは値だけでなくlineageを確認する。A→B→Aと偶然値が一致しても、無効な古いfillを正当化しない。新しいBを公開後、古いAのcache_writeが同じslotへ入った場合は、bindが変わっていなくても旧Bのvisibilityを残してはならない。別childの古いpublishで直近physical内容を上書きしたことにしてはいけない。

read errorやshort IOから成功hitを作れない。cache failure後は明示したorigin fallback childから結果を返す。origin failureを旧cache/zeroでokへ変えない。ただし正当なcache hitがoriginを読まなかった場合、未観測のorigin故障を検知済みとはしない。

## 5. 所有権・cancel・exactly-once

下位forward ledgerが持つphysical leaseはactual completionでのみ解放できる。cancel_request/cancel_io、timeoutを模したepoch invalidation、parent commit、metadata invalidation、source freshness失効では解放しない。

- leaseが残るslotへ成功bindや競合cache_writeをclaimした時点で`LIVE_LEASE_REUSED`。実際のlate overwriteまで待って見逃さない。
- cache_read child buffer、source snapshot、caller resultは独立所有。late cache completion後にcaller resultを書き換えれば`RESULT_MUTATED`。
- 同じ親への二度目のcaller通知は`DUPLICATE_RESULT`。input自体が二度目のcommit eventならsemantic errorも別に記録可能だが、buggy candidateが一つのeventから二通知した違反をparserだけで消さない。
- 原本mutationに対応actual child completionがないのにcaller完了をclaimすれば`EARLY_ACK`。lower status/countを変えれば`LOWER_RESULT_CHANGED`。
- retire/resumeの安全性はactual ledgerの未終了集合で判断する。資源不足/枯渇で既存childやwriterを捨てない。

owned cellsの予約・保持はD02 §4.1の上限に従う。oracle自身の履歴もbounded。固定origin arrays、slot arrays、request/io recordsを別計上し、cell-per-heap allocationを避ける。retireしたcaller result/historyを消して予算を回避しない。純粋host RAM計測をnonpaged実測とは呼ばない。

## 6. durability / flush / FUA

ordinary completionはdurabilityを増やさない。失敗write/discardでも適用済みeffectは残り、persistし得る。reported prefixを超える実effectもhistoryへ残す。

- flush frontierはflush parent admissionより前にadmitされたwrite/discard親集合。rejected親は除外し、後からadmitされたmutationは加えない。
- D02 referenceはfrontier内の全origin childがactual completeするまでorigin_flush submitをpendingにする。frontierに未forward mutationがあっても「childなしだから完了」としない。
- origin_flushのok completionはfrontierの全applied effectsがbyteごとにdurable versionでcoverされることを必要とする。上書きされたbyteがより新しいdurable versionならcoverできる。未適用の要求領域まで架空のpersistを要求しない。
- FUA writeのok completionはreported prefixの適用済みversionsがdurableでcoverされることを必要とする。失敗時にFUA成功の保証を作らない。
- 原本lowerのreported prefixにsample/apply coverageがない場合、あるいはflush/FUAの成功保証をlowerが満たさない場合は**backend-contract violation**。candidateの安全性PASSや通常のrejected decisionへ丸めない。

cache childはsource/physical full coverageをpublish/hit条件として別に検査する。不十分なcache転送を単にlower reported fullという入力だけで適格にしない。cache短転送の反例をorigin backend違反と取り違えて、publication検査へ届かなくすることも避ける。

## 7. semantic error / safety violation / assertion

最初の失敗event indexで停止し、後続eventを実行したように集計しない。同一eventでは、input lifecycle不成立→lower promise違反→candidate安全違反→trace assertion不一致の順に検査する。ただしcandidateの不正claimをinput phase errorへ読み替えない。errorの公開表示は固定code/indexだけで、payload/パス/生serde文言を出さない。

| kind | 例 | 意味 |
|---|---|---|
| input_error | F05のfixed parser code | typed traceを構成できない |
| semantic_error | DUPLICATE_COMPLETION, TRANSFER_AFTER_COMPLETION, TRANSFER_OVERLAP, UNACCEPTED_IO, INVALID_PHASE, POST_POWER_LOSS | staticに整形式だがschedulerのphase/操作が不成立 |
| backend_violation | READ_REPORTED_UNSAMPLED, WRITE_REPORTED_UNAPPLIED, FUA_NOT_DURABLE, FLUSH_NOT_DURABLE | 模擬下位装置の提示した成功/countが物理履歴と矛盾 |
| safety_violation | STALE_PUBLICATION, STALE_CACHE_RESULT, EARLY_ACK, LOWER_RESULT_CHANGED, LIVE_LEASE_REUSED, RESULT_MUTATED, DUPLICATE_RESULT, INVALID_OWNERSHIP | candidateの危険なclaimを独立履歴が検出 |
| assertion_failure | ASSERT_READ, ASSERT_RESULT, ASSERT_CACHE, ASSERT_DECISION, ASSERT_ORIGIN | 手書き期待値と観測が一致しない |

具体codeはF06のenumで固定し、上表のcategoryを変更する追加はD07再review。入力の意味が不明なとき「安全のため何でもreject」で合格にしない。ordinary errorの正常伝播（たとえばio_error/report0をそのまま返す）はmodel FAILではない。

## 8. 小さい固定例（実行結果ではなく規範）

記号: `W`=write admission、`T`=実transfer、`C`=actual completion、`P`=persist、`R`=sample、`ACK`=caller commit。byte値は例だけで、全range atomicityを意味しない。

| ID | 入力/順序 | 期待する判定 |
|---|---|---|
| O01 | 初期A→W(B)→T(B)→C(ok,1)→ACK→power_loss、Pなし | callerは成功だがpower-loss後A。ordinary successを耐久性としない |
| O02 | 初期AA→W(BB)→byte0だけT→C(io_error,0)→ACK | volatileはBA、status/countはio_error/0。両要求byteの旧cacheを復活させない |
| O03 | W(B)のT v1→W(C)のT v2→P(v2)→P(v1)→power_loss | C。旧persistでBへ退行不可 |
| O04 | discard2bytes→T/C→byte0へB write | byte0 known B、byte1 unknown。zero値をknownとして捏造不可 |
| O05 | 初期AA→read byte0→writeBB→read byte1→C/ACK | sampled `[A,B]`を許す。commit時origin比較は禁止 |
| O06 | FUA W(B)→T→C(ok,1)、Pなし | backend_violation FUA_NOT_DURABLE、candidate PASSではない |
| O07 | W(B)admit→flush admit→flush child submit | writerが未forward/未完了ならpending。新io IDで後でretry |
| O08 | W(B)→T/C→flush child C(ok,0)、Pなし | backend_violation FLUSH_NOT_DURABLE |
| O09 | W(B)T/C→flush admit→W(C)T/C/P→flush C | newer durable Cはfrontier Bをcover。後続Wはfrontierには加えない |
| O10 | cache child error→origin child R/C(io_error,1)→ACK | originのerror/count/sampleをそのまま。cache okへ置換不可 |
| O11 | fill Aをsample→W(B)T/C→古いAをcacheへT/C→publish | correctはrejected。stale-fill mutantのaccepted claimはSTALE_PUBLICATION |
| O12 | W(B)admit/forward→actual C前にACK | correctはpending。early-ACK mutantはEARLY_ACK |
| O13 | cache_write submitがaccepted/forwarded、actual completion前（write leaseあり）→cancelまたはepoch→同slotをbind | correctはpending。lease早期解放mutantの成功bind claimはLIVE_LEASE_REUSED。submit decision=pending（未forward）のケースではない |
| O14 | B fill完了→A fillが同slotを上書き→B publish | Bの古いcontent ownerでは公開不可。AがerrorでもBを復活不可 |
| O15 | writer2個admit→一方C→fill/hit | 残writerがある間unsafe admission不可 |
| O16 | accepted/forwardedでactual completionのないchildを残してtrace終端 | safety違反未検出でもINCONCLUSIVE。drain成功へ丸めない |
| O17 | events=[]またはassertだけで独立観測なし | INCONCLUSIVE。0 applicable testsはPASSではない |
| O18 | power_loss時に未完了child/parentあり | abandoned数を記録。成功completionを生成せず、以後assertだけ |

F06は同一O11/O12/O13 scheduleをreferenceと各mutantへ与える。referenceの期待decision、mutantの**独立safety code**をtestが検査する。parser拒否、expected assertionだけの失敗、panic、未実行をmutant検出成功と扱わない。D07文書を書いただけでこれらを実行済みとはしない。

## 9. 証拠schema / verdict

[evidence-v1.schema.json](../../spec/evidence-v1.schema.json) は**F05成功後**のReplayResult reportのshape。全field必須、未知/重複key禁止、versionはinteger token。count/seed/indexはcanonical u64 decimal string。parse失敗は固定TraceErrorを別に返し、ReplayResultを作らない。seed/event countを0と捏造してinput_errorのevidenceを作らない。reportはpayloadや生device/path情報を含めない。

`trace_digest`は**元のtrace入力のUTF-8 bytes全体のSHA-256、小文字hex**。空白/newline/key orderを正規化せず、再serializeしたTraceV1をhashしない。typed TraceV1だけを受け取るpure `run_trace`は元bytesを持たないためnullとし、元bytesを保持するCLI/試験runnerがその同一入力をhashできた場合だけ補える。`source_revision`は実行したreference/oracleをbuildした**正確なGit commit**で、検証可能なbuild provenanceから供給する。たまたま現在のcwdのHEAD、ユーザーが渡した無検証文字列、未反映のdirty sourceがあればnull。pure model内でfilesystem/Git/networkを検索しない。形の正しいhexだけではこの条件を満たさず、採録したrunner/CIの証拠との照合が必要。

- mode=reference / mutant、fault=none / stale_fill / early_ack / late_slot_reuse。mode/faultの対応を固定する。将来faultを増やすときはschemaとtestを一緒にreviewする。
- `checks`はorigin_results、cache_lineage、ownership、durability、trace_assertions、shadow。各passed/failedのcountとstatusを持つ。未実施はnot_observed、v1にeventがないshadowはnot_applicable。
- `meaningful_observations`は独立oracleの適用可能な検査回数。assertだけを足さない。origin result、cache lineage、lease/ownership、durabilityの検査で増える。
- `terminal`はdrained / pending / power_loss / stopped。通常終端でactual未完了IOまたはadmit済み非fill親の未commitがあればpending。fillのcaller ACKを要求しない。未retireのrecordが残ることとactual IO未完了を混同しない。
- `pending_requests/ios`は現sessionの未完了数。power_lossでは直前の未完了数をabandonedへ移し、pendingは0。既に完了済みのcaller/IOをabandonedへ数えない。abandonedは成功ではない。
- error/violation/assertion failureがあればoutcome=failed、violation非null、terminal=stopped（power-loss後のassert failureもstopped）。errorなしでもpending、meaningful=0ならinconclusive。drainedまたはpower_lossでmeaningful>0かつ違反なしのときだけpassed。
- mutantを意図通り検出したrunのoutcomeも**failedのまま**。外側のunit testが「狙ったcodeで失敗した」と検査するのであって、危険candidateをpassed表示しない。
- schemaだけでcount合計、phase、unknown mask、source identityを保証できない。F06のencoder/result testが相互整合を確認する。report比較に時刻/duration/random環境値を入れない。
- `kernel_runtime_tested=false`、`production_qualified=false` はv1で常にfalse。portable evidenceを実機合格へ昇格しない。

passedは**その固定traceと観測カテゴリだけ**に対する値。他カテゴリnot_observedを隠さず、fixture suiteの必要coverageをF07/F08/F09で別判定する。source/digestがnullのローカル実験は再現情報不十分としてrelease gateの証拠にしない。

### 9.1 固定する集計単位

一つの表の評価点について**1回**のcheckを数える。byte数/内部predicate数では増やさない。同じeventから異なるcategoryの評価が生じることはある。**§7のstage順（input lifecycle→backend promise→candidate safety→assertion）を先に適用**し、同じstage内だけcategory順（origin_results→durability→ownership→cache_lineage→trace_assertions）で評価する。origin_resultsのlower completion検査とdurabilityのFUA/flush検査はbackend stage、origin caller commitの検査はcandidate safety stage。後者がFUA/flush backend違反を先取りしない。最初の失敗で停止し、まだ評価していない後段checkをfailed/passedに数えない。

| category | ちょうど1 checkを行う評価点 |
|---|---|
| origin_results | origin系io_complete時のlower status/count/prefix coverage検査。およびorigin childからのcaller commit claim時の実終了・exactly-once・lower結果保持検査（別の評価点） |
| durability | FUA write/flushのok io_complete時のdurable保証。validな非zero origin_persist時のversion/history反映検査。実mutation historyがあるpower_loss時のvolatile→durable一致検査 |
| ownership | accepted child forward claim（target/source descriptorとactual lease所有）、accepted slot_bind/retire_request/resume_cache claim。それぞれdescriptor/集合全体を1回として検査。さらに下記post-event retained-result guard |
| cache_lineage | accepted cache_read admission、accepted cache_publish、cache childからのaccepted caller commit claim。それぞれlineage/knownness/freshness/immutable resultの全条件を1回として検査 |
| trace_assertions | 実行した各assert eventを1回。失敗したassertも含める |
| shadow | v1には評価点なし。両count=0、status=not_applicable |

正常にrejected/pendingとなったclaimには成功検査を捏造しない。危険mutantがaccepted claimを出した評価点ではfailedを1増やす。inputのphaseが成立せず評価点まで到達しなければ、そのcheckは0回のまま。下位origin完了のprefix違反はorigin_resultsのfailed、FUA/flush durable違反はdurabilityのfailed。前節のcategory errorとこの集計は別軸で、backend_violationを安全性passへ変えない。

**post-event retained-result guard:** 各非assert eventの全通常stage/checkが成功した後、immutable caller resultが一つでも存在すれば、それら**全体を一つのownership check**としてoracleの独立snapshotと比較する。新規commit直後も、late cache transfer/completion、cancel、epoch、retire、power_loss後も省略しない。candidateのresult version/同じbufferへの参照を正解にせず、status/count/bytes/known mask/既存resultの存在を確認する。相違はRESULT_MUTATEDでfailedを1増やして停止する。先行checkが既に失敗したevent、またはresultがまだ一つもないeventでは0回。これによりcommit時点だけでなく、後からのbuffer書き換えも必ず観測する。guardはcategory順の通常checkを終えた後の明示checkpointであり、結果数/byte数によってcountを増やさない。

各categoryは`failed>0 → status=failed`、それ以外で`passed>0 → passed`、両方0ならnot_observed。shadowだけnot_applicable/0/0。`meaningful_observations = Σ(passed+failed)`（origin_results/cache_lineage/ownership/durabilityの4カテゴリ）、`assertions_checked = trace_assertions.passed + trace_assertions.failed`。assert-onlyやunsupported shadowをmeaningfulへ加えない。

`events_total`はtyped trace配列長（最大10,000）。`events_executed`は開始したeventの数で、最初の失敗eventも含む。失敗index=iならexecuted=i+1、正常終端ならexecuted=total。失敗時に未実行の残りevent、未実行assertを加算しない。違反indexは必ずその開始済みeventを指す。atomicに適用できなかったeventの内部効果まで成功したという意味ではない。

F06はJSON Schemaに加え、数値が`18446744073709551615`以下、canonical string、重複key、integer **token**、上記count/status/outcomeの相互式を検査する。schemaの20桁regexだけではu64上限を保証できない。最初のcodec実装では固定正常reportと意図的なcount/outcome矛盾reportを別々に試験する。

## 10. shadow比較と限界

v1にはshadow_read eventがないためshadow欄はnot_applicable。将来Windows shadowでは、比較対象が同一range・同一安定区間に属することを独立write履歴で確認する。sample間の合法な同時writeで違う値が出た場合はunobserved/inconclusiveとして再試験し、一致したことにも破損したことにもしない。candidateのgenerationだけを安定区間の根拠にしない。

未モデル化: bit rot、sector atomicity/firmware、flushを偽る実device、raw mutation捕捉漏れ、実cancel/DMA/hang、PnP/IRQL/電源管理、暗号化/認可。D01/D04/D10とlab gatesは残る。既存finite checker、F06reference、D03 candidateが互いに一致しても、独立履歴のoracleや実Windows mappingを省略しない。
