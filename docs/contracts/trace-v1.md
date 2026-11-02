# Portable trace v1 — D02

状態: D02 review待ち。これはin-memory schedulerの契約であり、kernel ABIでも実device trace decoderでもない。F05はこの版の受入後に実装する。

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
| owned in-flight buffer bytes | 合計16,777,216。原本状態・bounded履歴とは別に計上 |
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
- publishはsource full-known coverage、physical full-write coverage、success/full count、epoch/generation一致、writer=0、active、slot/line/incarnation/直近fill provenance一致をすべて必要とする。completionからpublishの間のslot再bindも検出する。
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

cache admission条件やlease/resource不足によりio_submit/slot_bind/fill_begin/cache_publish/read_commit/resume_cacheがrejected/pendingになることは**正常なmodel decision**。そのrequest/io IDは消費済みとして保存し、再利用しない。拒否された子へtransfer/completeするのはsemantic error。拒否した親への子submitもsemantic error。

各request/childは独立したterminal flagとbuffer ownerを持つ。parentが完了してもlate cache childのbuffer/leaseは残り、cache child完了でcallerへ二度通知しない。caller resultはimmutable copy（または同等のexclusive ownership移転）とする。

retire_requestは全accepted childがactual completeした後だけbufferを解放する。read/write/discard/flush親はcaller commit済みを必要とする。fill親はcaller resultを持たず、全child完了後に放棄/終了できる。pending childが残ればpending decision、leaseは保持。成功retire後の追加I/O/publish/commitは禁止。ID・固定result・宣言はtombstoneとしてtrace終端まで残し、assertは可能。公開済みcache entryは親bufferを参照せず、slot上のdataと独立したmetadataを所有する。

原本mutationのadmissionではforwardに必要な最小bookkeeping/child枠を予約する。未forwardのorigin submitが遅れてもwriter countを消さず、資源不足を理由に既存writerを見失わない。拒否された親をretireする場合は未forwardであることを検査し、登録していないwriterを減算しない。

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

flush_submitは、それより前にadmitされたwrite/discard parent集合をfrontierとして保持する。このportable referenceは、frontierの全origin childがactual completeするまでorigin_flush submitをpending/refusedにし、forward前の順序を保守的に直列化する。後続mutationはfrontierに加えない。flush childの成功completeにはfrontierの全applied effectsをdurableがcoverすることを必要とする。FUA writeの成功completeにもreported prefixのapplied versionsのdurable coverageを必要とする。ハードウェアがこの約束を破った入力はD07のbackend-contract violationでありcacheの成功ではない。

epoch_invalidateはadmission停止・全entry失効・checked epoch増分のみ。pending request/child/lease/writer countは保持。origin passthroughは継続できる。resume_cacheは全old childがactual completeし、全writerが0、counter未枯渇のときだけcold開始。identity再検証はこのportable abstractionの外の必須条件（D05）；このeventだけで実activation可とはしない。

power_lossは**whole-sessionの終端**。volatileをdurableへ戻し、cacheを全破棄する。未完了要求はabandonedと記録し完了成功にしない。以降はassert系だけを許す。これは実行中driverがhangしたDMAを解放する手段ではなく、同session内のrestart/resumeでもない。

## 7. parser / replay / oracleの責務

F05: strict shape/token/limits、rangeとgeometry、全IDの一意性と先行参照、parent/target対応、slot/sourceの型と同一親、static child-range、assert data/mask長、assert indexが先行eventであることを検査する。coherence、actual completion phase、accepted attempt枠、lease成功判定、assertの真偽は実行しない。

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

各variantの正常fragmentは§3。不正例は`request:1`、`length:"01"`、`offset:"-1"`、`slot:false`、`status:"timeout"`、`known:"2"`、未知event/余分key/重複key、未declaration参照でF05拒否。`io_complete`を二回、complete後transfer、未完了mutation_commit、pending leaseで強制成功したrebind、stale publishを正しい結果としてassertする等はreplay/oracle側のFAILであり、入力parserにcoherence判断を押し付けない。

## 9. 反例と限界

[有限checker](../../experiments/coherence/README.md)にstale-fill、late-token、two writers、partial/failed write、short/unknown fill、hit race、counter wrap、epoch/cancelでのlease早期解放、late二重completionを残す。正常モデルが境界内で全探索され、意図的故障が具体的witnessを生むことを要求する。

cache media corruption、暗号化、実IRP/MDL/DMA、hardware持続hang、実flush耐久性はこのcheckerでは未実証。epoch/generationはbit rotのchecksumではない。trace/reference modelと製品engineが一致しただけで独立oracleの代わりにしない。
