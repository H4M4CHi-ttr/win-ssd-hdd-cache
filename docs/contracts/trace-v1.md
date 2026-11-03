# Portable trace v1 — D02

状態: D02独立review受入済み、PR #67でmain統合済み。これはin-memory schedulerの契約であり、kernel ABIでも実device trace decoderでもない。F05はこの凍結版のstrict parserを実装する。受入範囲は[統合証拠](../evidence/2026-11-02-main-integration.md)を参照。

## 1. Envelope / 型 / 上限

```json
{"schema_version":1,"seed":"0","capacity":"2","line_size":4096,"slot_count":1,"counter_limit":"18446744073709551615","initial_hex":"0000","events":[]}
```

全field必須、未知/重複key禁止、object-only、末尾の別document/BOM禁止。`schema_version`は整数token `1` のみ（`1.0`/`1e0`/bool不可）。`line_size`は4096/8192/16384/32768/65536、`slot_count`は整数token1〜256。その他のunsigned値はcanonical decimal **string**（`0`または非zero開始、u64以下）。IDは正数、slot/line/index/offsetは0を許す。hexは小文字 `[0-9a-f]` の偶数長、prefix/separatorなし。

| resource | 最大 / 規則 |
|---|---|
| raw input | 67,108,864 UTF-8 bytes。parse前に検査 |
| capacity | 1,048,576 bytes、0可。initial_hexはちょうどcapacity bytes |
| events | 10,000。empty可、ただしempty traceをqualification成功件数に数えない |
| decoded payload | initial_hex・全write data・全assert dataの合計16,777,216 bytes |
| individual payload / range | capacity以内、加算前に`length <= capacity-offset` |
| live parent / child I/O | 各1,024。拒否は原本へforwardする前。既存childを捨てない |
| owned data cells（予約を含む） | 合計16,777,216。in-flight buffer、retained result、source snapshot、mutation historyを含む。詳細は§4 |
| counter_limit | 1〜u64::MAX。小さい値はexhaustion反例用で製品設定ではない |

JSON Schemaは[trace-v1.schema.json](../../spec/trace-v1.schema.json)。JSON Schemaだけでは重複key・integer **token**・u64文字列上限・reference/phaseを保証できないためRust parser/replayの規則も必須。入力String自体はcallerが既に所有している。上限を全allocationがゼロという意味にしない。

seedは記録用で、v1のevent順は配列だけで決まる。hidden RNG/時刻/thread schedulerなし。親request IDとchild I/O IDは別namespace、それぞれtrace全体で再利用禁止。slot IDは0〜slot_count-1。derived generation/count/epoch/bytesをtraceから上書きしない。

## 2. State / linearization

initial originは全known、volatile=durable=initial_hex。epoch=1、全line generation=1、cache admission有効かつ全entry無効。slotはunbound/incarnation=0。これは承認済みの抽象model session開始を表すだけで、実driver activation許可ではない。

line extentは `[line*line_size, min(capacity,(line+1)*line_size))`。最終short lineは実在するextentだけを対象にし、zero paddingしない。capacity=0にcacheable lineはない。

- mutation parentのadmissionで全交差lineのvisibilityを無効化し、generationをchecked増分し、各lineのactive-writer数を増やす。範囲はreported completionではなく**要求範囲全体**。
- active writerはactual origin child completionまで残す。cancel、timeout、error、部分変更でも古いentryを復活させない。未forward親を拒否したときはwriterを登録しない。
- fill parentのadmission前にwriter=0を要求し、**最初のorigin read sampleより前**のepoch/generationを保存する。source read完了時に新tokenを作り直さない。
- hitは一つのline内のrequestだけ（cross-line readはoriginへ）。metadata検証時にread leaseとtokenを取得し、child専用bufferへ読む。最終検証とimmutable caller resultのcommitを一つのtransitionにする。
- commit後の実通知までにwriterが来ても、既にcommitしたsnapshotを変えない。origin-readのfragment sampleは各eventで観測した値を返せ、後のwrite完了時のoriginに合わせない。
- cache read leaseはshared、fill write leaseはexclusive。実転送が終わるactual completionまでslot再bind禁止。metadata invalidationはleaseを解放しない。
- **cache_writeのaccepted admissionごとに**、同じlineへの同じslotでも、物理転送より前にslotのvisibilityを失効し、incarnationをchecked増分する。この新incarnationとchild IO IDをcontent provenanceとして記録する。exclusive lease取得・失効・provenance更新は一つのtransition。枯渇時はslotをretireしてrejectし、転送しない。
- publishはsource full-known coverage、physical full-write coverage、success/full count、epoch/generation一致、writer=0、active、slot/line/incarnation/直近fill provenance一致をすべて必要とする。completionからpublishの間のslot再bindも検出する。
- publish時のprovenanceはそのcache_write admissionの値と完全一致し、slotにactive write leaseがないことを要する。後発writeが失敗/短転送になっても旧visibilityを復元しない。古いfillのpublishを拒否するだけでは、既に上書きされた別fillのvalid metadataは救えない。
- generation枯渇はline permanently uncacheable、incarnation枯渇はslot retired、epoch枯渇はsession cache-disabled。counterをwrapしない。まだ転送していないrequestの資源不足は拒否し、既存I/Oの追跡を削らない。

metadata transitionは抽象的にatomic。実装にspinlockを保持したままI/O待ちを要求しているのではない。対応する実際のlock/IRQL/ownershipはD04/D10未完了。

## 3. 全eventのwire形

下表のJSONは各variantの全fieldを示す**fragment**。単独で有効なtraceとは限らず、先行declaration/phaseが必要。記載field以外や欠落はschema error。`request`/`io`/`slot`/`line`等はu64 decimal string。

| event | 正常な形（例） |
|---|---|
| read_submit | `{"event":"read_submit","request":"1","offset":"0","length":"2"}` |
| write_submit | `{"event":"write_submit","request":"2","offset":"0","data":"0102","fua":false}` |
| discard_submit | `{"event":"discard_submit","request":"3","offset":"0","length":"2"}` |
| flush_submit | `{"event":"flush_submit","request":"4"}` |
| fill_begin | `{"event":"fill_begin","request":"5","line":"0"}` |
| io_submit | `{"event":"io_submit","io":"1","request":"1","target":"origin_read","slot":null,"source":null}` |
| io_transfer | `{"event":"io_transfer","io":"1","offset":"0","length":"2"}` |
| io_complete | `{"event":"io_complete","io":"1","status":"ok","reported":"2"}` |
| cancel_request | `{"event":"cancel_request","request":"1"}` |
| cancel_io | `{"event":"cancel_io","io":"1"}` |
| retire_request | `{"event":"retire_request","request":"1"}` |
| cache_publish | `{"event":"cache_publish","io":"3"}` |
| read_commit | `{"event":"read_commit","request":"1","io":"1"}` |
| mutation_commit | `{"event":"mutation_commit","request":"2","io":"2"}` |
| epoch_invalidate | `{"event":"epoch_invalidate","reason":"disable"}` |
| slot_bind | `{"event":"slot_bind","slot":"0","line":"0"}` |
| resume_cache | `{"event":"resume_cache"}` |
| origin_persist | `{"event":"origin_persist","io":"2","offset":"0","length":"2"}` |
| power_loss | `{"event":"power_loss"}` |
| assert_read | `{"event":"assert_read","request":"1","data":"0000","known":"11"}` |
| assert_result | `{"event":"assert_result","request":"2","status":"ok","reported":"2"}` |
| assert_cache | `{"event":"assert_cache","slot":"0","valid":false}` |
| assert_decision | `{"event":"assert_decision","index":"0","decision":"accepted"}` |
| assert_origin | `{"event":"assert_origin","offset":"0","data":"0000","known":"11"}` |

`target`: origin_read/origin_write/origin_discard/origin_flush/cache_read/cache_write。`status`: ok/io_error/cancelled。`reason`: disable/suspend/target_change/fault。`decision`: accepted/rejected/pending。nullは明示必須。knownはdataのdecoded byte数と同じ長さの`0`/`1`文字列。unknown位置のdataはcanonical zero placeholderであり、実際のzeroやerrorを意味しない。

## 4. declaration / child attempt

親の種別はread/write/discard/flush/fill。`io_submit`のtarget対応は、read→origin_readまたはcache_read、write→origin_write、discard→origin_discard、flush→origin_flush、fill→origin_readの後にcache_write。v1は各親の同一targetにつき**一つのaccepted attemptだけ**。拒否/pendingで未forwardのsubmitはその枠を消費せず、後で新IO IDによる再試行を許す。readはcacheとoriginの二つが同時pendingになり得る。新規retryを同じIDで表現しない。cacheの失敗/cancel/freshness拒否後にorigin fallbackを明示submitする。

origin targetはslot/sourceともnull。cache_readはbound slotとsource=null。cache_writeはbound slotと、同一fill親の完了済みorigin_read IOをsourceにする。origin_read IO rangeは親range、fillは完全line extent。cache_read rangeは親のline内部分、cache_writeはfull line extent。

cache admission条件やlease/resource不足によるrejected/pendingは**正常なmodel decision**（§4.2）。parent/childのdeclaration eventが拒否/pendingでもIDは消費済みで再利用しない。未admit親への子submit、未accepted childへのtransfer/completeはsemantic error。retryは新declaration IDを用いる。IDを宣言しないpending eventは同じ対象への再実行を許す。

各request/childは独立したterminal flagとbuffer ownerを持つ。parentが完了してもlate cache childのbuffer/leaseは残り、cache child完了でcallerへ二度通知しない。caller resultはimmutable copy（または同等のexclusive ownership移転）とする。

retire_requestは全accepted childがactual completeした後だけ解放可能なbufferを解放する。admit済みread/write/discard/flush親はcaller commit済みを必要とする。**未admitのrejected親は例外**で、accepted childがなくwriterも登録していないためcommitなしでretireできる。fill親はcaller resultを持たず、全child完了後に放棄/終了できる。pending childまたは必要commitが残ればpending decision、leaseは保持。成功retire後の追加I/O/publish/commit/cancelは禁止。ID・固定result・宣言はtombstoneとしてtrace終端まで残し、assertは可能。公開済みcache entryは親bufferを参照せず、slot上のdataと独立したmetadataを所有する。

原本mutationのadmissionではforwardに必要な最小bookkeeping/child枠を予約する。未forwardのorigin submitが遅れてもwriter countを消さず、資源不足を理由に既存writerを見失わない。拒否された親をretireする場合は未forwardであることを検査し、登録していないwriterを減算しない。

### 4.1. retained data / budget

owned data cellは「1 byteの値と、そのknownness/versionに必要な付帯情報」の論理単位。16,777,216 cellsは実RSSが16 MiBという意味ではない。packed array等を用い、1cellにつき最大16 bytes、管理recordはevents/live数の既定上限内とする。cellごとのheap allocationをしない。

- childのread/sample buffer、fill source、caller result、mutationのapplied-byte history、これらの**将来用予約**を合算する。同一cellのownership移転は二重計上しないが、copyを二つ保持すれば二つ数える。
- 読み親のadmission時にcaller resultの最大要求長を予約する。write/discard親のadmission時にそのIOの全要求長分のmutation historyを予約する。各childはforward前にread/source等のbufferを予約する。sourceをborrowする場合はそのownerをretireできるまで保持する。
- `retire_request`でもcaller resultとmutation historyは解放しない。後の`assert_read`と`origin_persist`が参照できる。後者はretire済みIOについても、実際に適用されたbytes/known mask/apply-versionだけを使う。未転送部分をpersistしない。
- parent admissionの予算不足はrejected、child admission時の不足はpending。予約できなければI/Oをforwardしない。commit/complete時に突然追加予算が必要にならないよう事前予約する。不要となった予約だけ解放し、結果/historyを暗黙に捨てない。
- 固定のvolatile/durable origin array（各capacity）、slot array（slot_count×line_size以下）、入力TraceV1 payloadはこのowned budgetとは別だが、各上限を持つ。history/結果をここへ移して予算を回避しない。

たとえば1MiBのreadをcommit/retireしてもresultの1MiB分は残る。後続のadmissionが上限に達したら拒否/pendingとなり、10,000件のtraceから無制限に結果を蓄積しない。

### 4.2. decision / lifecycle table

assert以外の全eventは正常実行時に一つのdecisionを残す。表の例外以外はaccepted。構造不正・存在しないphase・重複terminalはdecisionではなくsemantic errorでreplayを停止する。assert系はdecisionを生成せず、`assert_decision`がassert系を指す入力はF05のINVALID_REFERENCE。

| event / 条件 | 一意な結果と効果 |
|---|---|
| read/write/discard/flush_submit | 予算/live親枠不足ならrejected（未admit tombstone）。それ以外accepted。cache disabledでも原本requestを拒否しない |
| fill_begin | 上記資源不足、cache disabled、uncacheable line、writer>0ならrejected。otherwise acceptedでsource token固定 |
| io_submit origin系 | accepted親の正しいtargetで資源があればaccepted。child枠/buffer不足、flush frontier未完ならpending。declined childは未forwardで枠を消費しない |
| io_submit cache_read | disabled、invalid/unbound/別line/retired slot、writer>0、cross-line hitならrejected。これらを先に評価し、残るlease競合/live child/buffer不足はpending。acceptedだけread leaseと現在のtokenを取る |
| io_submit cache_write | disabled、unbound/別line/retired slotならrejected。sourceが未completeならpending、complete後の失敗/短転送/unknownならrejected。残るlease競合/live child/buffer不足はpending。incarnation枯渇はslotをretireしrejected。acceptedだけexclusive lease・visibility/provenanceを更新。sourceの古いgenerationはここでは拒否せずpublishで検査する（下記参照） |
| slot_bind | retired/exhaustedならrejected。physical lease中ならpending。それ以外acceptedでvisibility失効・incarnation増分。同じlineへのbindもno-opにしない |
| cache_publish | 未completeならpending。complete後に§2条件が一つでも不成立ならrejected、成立ならaccepted。一度accepted publish済みの同一IOの再publishはsemantic error（失効を取り消さない） |
| read_commit / mutation_commit | 子が未completeならpending。完了cache childの最終検査失敗はrejected。完了origin childはそのままaccepted。一度commit済み親の再commitはsemantic error |
| cancel_request / cancel_io | 未retire対象にflagを立てaccepted。反復、actual complete後、caller commit後もaccepted no-op。I/O完了・writer/lease解放・caller通知を生成しない |
| retire_request | §4のdrain/commitが足りなければpending。成立時accepted。一度retire済みならsemantic error |
| resume_cache | 既にactiveならaccepted no-op（purge/resumeをやり直さない）。epoch枯渇ならrejected。disabled中にold child/writerが残ればpending。それ以外accepted cold-start |
| epoch_invalidate | accepted。counter上限ならdisabled固定、wrapしない。既存ownershipは変えない |
| io_transfer / io_complete / origin_persist / power_loss | 各規則を満たす場合accepted。phase/coverage/durability違反はsemantic/oracle errorでありrejectedへ丸めない |

commit済み親に新childをsubmitすること、retire済み親への制御、既にaccepted済みの同targetのchild追加はsemantic error。既存late childのtransfer/actual completeとその安全なdrainだけはcaller commit後も許す。power_loss後はassertだけを許す。

v1はsource freshnessによるcache_writeの早期省略を行わない。これにより「既に古いsourceが物理slotへ到着しても、旧visibilityを残さず、publishを拒否する」という防御を明示的に試す。製品側の早期チェックは追加の最適化になり得るが、write admission時の失効/provenance更新やpublish時の最終検証を代替しない。

## 5. transfer / completion / commitment

`io_transfer`はchild-relative rangeの一つの適用または観測。sourceはmodelの実際のorigin/write payload/slot/source-read bufferから計算し、traceが成功bytesを注入しない。同一child内のnonzero transfer範囲は重複禁止。zero lengthは範囲内でno-op。flushはtransferなし。

- origin_write: payloadの該当fragmentをvolatile originへ適用。actual effectとreported countは別。
- origin_discard: 該当fragmentをunknownへ変更（ゼロ埋めしない）。
- origin_read: 各fragmentの現在のbytes/known maskをowned bufferへsample。
- cache_write: full-known source bufferからslotへcopyし、actual coverageを追跡。
- cache_read: slotのfragmentをchild bufferへsample。slot incarnation/read leaseはcompletionまで固定。

複数byte fragmentは一つのモデルstepとして扱うがWindowsのatomicity保証ではない。tiny checkerは1byteずつのinterleavingを探索する。実際の非atomic転送は細かいfragmentに分けて表現できる。

`io_complete`はactual終端。以後transfer/再completeはsemantic error。reportedは要求length以下、flushは0。origin readのreported prefixにはsample済みcoverageが必要、unknownは許される。write/discardの失敗はreportedが0でも適用済みfragmentを元に戻さない。成功の場合もfull countと仮定せず、reported prefixが実際に適用済みであることを要する。cache短転送/失敗/unknown sourceからpublishや成功hitを作らない。

`read_commit`は完了済みread childから一度だけcaller resultを確定する。cache sourceなら最終token/entry/slot/full requested coverage/knownnessを再検証し、失効ならrejected（origin fallbackが必要）。origin sourceならそのlower status/reported countとsample bytes/maskをそのまま保存する。origin errorをcache/zeroで成功にしない。cache hitが下位originを読まない場合の潜在故障まで検出したとは主張しない。

`mutation_commit`は対応する完了済みorigin childのstatus/reportedを変更せず一度だけ保存する。write admissionやapplyだけでcaller成功を返すのはearly ACK。cancel_request/cancel_ioはflagだけでactual completionを生成しない。

## 6. durability / flush / session

各origin mutation fragmentに単調なapply-versionを付け、volatile状態とは別のdurable byte historyを持つ。ordinary completionはpersistではない。`origin_persist`は指定mutation IOの適用済みfragmentだけをdurableへ反映する。byteごとのdurable versionを後退させない。より新しいversionが既にdurableなら古いpersistはno-op。write/discard errorの適用済み効果もpersistし得る。

flush_submitは、それより前にadmitされたwrite/discard parent集合をfrontierとして保持する。このportable referenceは、frontierの全origin childがactual completeするまでorigin_flush submitをpendingにし、forward前の順序を保守的に直列化する。後続mutationはfrontierに加えない。flush childの成功completeにはfrontierの全applied effectsをdurableがcoverすることを必要とする。FUA writeの成功completeにもreported prefixのapplied versionsのdurable coverageを必要とする。ハードウェアがこの約束を破った入力はD07のbackend-contract violationでありcacheの成功ではない。

epoch_invalidateはadmission停止・全entry失効・checked epoch増分のみ。pending request/child/lease/writer countは保持。origin passthroughは継続できる。resume_cacheは全old childがactual completeし、全writerが0、counter未枯渇のときだけcold開始。identity再検証はこのportable abstractionの外の必須条件（D05）；このeventだけで実activation可とはしない。

power_lossは**whole-sessionの終端**。volatileをdurableへ戻し、cacheを全破棄する。未完了要求はabandonedと記録し完了成功にしない。以降はassert系だけを許す。これは実行中driverがhangしたDMAを解放する手段ではなく、同session内のrestart/resumeでもない。

## 7. parser / replay / oracleの責務

F05: strict shape/token/limits、rangeとgeometry、全IDの一意性と先行参照、parent/target対応、slot/sourceの型と同一親、static child-range、assert data/mask長、assert indexが先行する非assert eventであることを検査する。coherence、actual completion phase、accepted attempt枠、lease成功判定、assertの真偽は実行しない。

F06: phase/order、disjoint fragment、rundown/coverage、model decisionsとassertを実行する。duplicate terminal/未知phaseを固定semantic errorにする。入力の不正とcacheの安全違反は別の結果で、両方ともqualificationはFAIL。D07はcandidateのgeneration/valid bitを信用しない独立したorigin history/ownership/result oracleを定義する。

public parser APIは `parse_trace(&str) -> Result<TraceV1, TraceError>`。TraceErrorは固定code（INPUT_TOO_LARGE/INVALID_JSON/INVALID_SCHEMA/INVALID_VALUE/LIMIT_EXCEEDED/INVALID_REFERENCE）と`event_index: Option<usize>`のみ。元payload/パス/serde messageを含めない。構造errorのindexはnullを許す。semantic errorは別のReplayResultで最初のevent index/codeを報告する。

assert_read/resultはcommit済み親だけ、assert_cacheは現在のvisibility、assert_decisionは指定eventの直後に得たdecision、assert_originは現在の原本bytes/maskを比較する。unknown maskは正確に一致させ、any-valueの逃げ道にしない。

## 8. 正常traceと異常の分類

上記envelopeのeventsを以下にすると2byte origin readが完結する。

```json
[
  {"event":"read_submit","request":"1","offset":"0","length":"2"},
  {"event":"io_submit","io":"1","request":"1","target":"origin_read","slot":null,"source":null},
  {"event":"io_transfer","io":"1","offset":"0","length":"2"},
  {"event":"io_complete","io":"1","status":"ok","reported":"2"},
  {"event":"read_commit","request":"1","io":"1"},
  {"event":"assert_read","request":"1","data":"0000","known":"11"}
]
```

各variantの正常fragmentは§3。不正例は`request:1`、`length:"01"`、`offset:"-1"`、`slot:false`、`status:"timeout"`、`known:"2"`、未知event/余分key/重複key、未declaration参照でF05拒否。`io_complete`を二回、complete後transfer、未完了mutation_commitのpendingをacceptedとassert、pending leaseで強制成功したrebind、stale publishを正しい結果としてassertする等はreplay/oracle側のFAILであり、入力parserにcoherence判断を押し付けない。

## 9. 反例と限界

[有限checker](../../experiments/coherence/README.md)にstale-fill、late-token、two writers、partial write effects、short/unknown fill、hit race、counter wrap、epoch/cancelでのlease早期解放、late二重completion、二つのfillによる物理上書き/provenance反例を残す。partial effectsと最終error statusの伝播は別で、後者の実装検証はD07/F06。正常モデルが境界内で全探索され、意図的故障が具体的witnessを生むことを要求する。

cache media corruption、暗号化、実IRP/MDL/DMA、hardware持続hang、実flush耐久性はこのcheckerでは未実証。epoch/generationはbit rotのchecksumではない。trace/reference modelと製品engineが一致しただけで独立oracleの代わりにしない。
