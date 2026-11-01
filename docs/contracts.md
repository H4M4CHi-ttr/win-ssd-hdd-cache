# インターフェース契約 v0.1

状態: 設計。以下のRust APIとpathは**これから実装する契約**。kernel ABIを推測して実装しない。

## 1. Workspaceと依存

主実装はRust。host側はedition 2024。F01で利用可能なstable Rustの正確なversionを `rust-toolchain.toml` にpinし、実際に試験したMSRVを `Cargo.toml` に記録する。floating `stable` だけのCIを再現性の根拠にしない。Cargo.lockをcommitする。

planned crates:

| crate / path | 責務 | unsafe |
|---|---|---|
| `crates/cache-types` | checked range、明示的schema、状態・counter型 | 禁止 |
| `crates/cache-model` | in-memory origin、決定的queue/oracle/replay | 禁止 |
| `crates/cache-cli` | status/benchmark/diagnostics、後段service client | FFIを直接置かない |
| `crates/cache-service` | Windows inventory、認可済みIPC、管理 | OS binding境界だけ |
| `crates/cache-engine` | 選定engine adapter | D09/D04で隔離 |
| `drivers/windows` | Windows driver、必要に応じRust no_std/C WDK shim | D03/D04で管理 |
| `xtask` | repo整合性検査などの開発補助 | 禁止 |
| `apps/gui` | Tauri + Vue/TypeScriptの非特権UI | raw I/O禁止 |

host testsにWDK、管理者権限、実disk、network、署名鍵を要求しない。host crateはWindows/Linuxの両方で試験。最初のF01ではcache-typesのrangeとworkspaceだけを作り、未使用crateの空殻を大量に作らない。依存crateは必要なtaskで追加する。各taskのFilesは主要pathを列挙しており、そのtaskに必要なworkspace登録/Cargo.lock/lib.rs export/test登録は付随変更として許可する。無関係な依存更新は行わず、共有ファイルの統合順を調整する。

serde/serde_json等は用途とlicenseを記録してpin。unsafeな自作JSON parserや勝手なJSON Valueへのroundtripで重複キーを消す処理は避ける。GUIのexact Tauri/Node/package versionsはU01でpinし、kernel toolchainとは独立に管理する。

## 2. 固定契約: byte range（F01）

```rust
pub struct ByteRange { pub start: u64, pub end: u64 }
pub struct LineSpan { pub first: u64, pub count: u64 }
pub enum RangeError { OffsetPastEnd, LengthPastEnd }
pub enum LineSize { K4, K8, K16, K32, K64 }
pub fn checked_range(offset: u64, length: u64, capacity: u64)
    -> Result<ByteRange, RangeError>;
pub fn line_span(offset: u64, length: u64, capacity: u64, size: LineSize)
    -> Result<LineSpan, RangeError>;
```

- 半開区間[start,end)。offset > capacityはOffsetPastEnd。length > capacity - offsetはLengthPastEnd。チェック前にoffset+lengthしない。
- zero lengthはEOFまで許す。line_spanのfirst=offset/line_bytes、count=0。
- 非zeroはfirst=offset/line_bytes、last=(end-1)/line_bytes、count=last-first+1。
- 4095,2,8192,K4 → first=0,count=2。8192,0,8192 → start=end=8192。
- capacity=u64::MAXも数学上は許可する。実デバイスとして利用可能という意味ではない。
- JSONの負数・浮動小数・boolの拒否はparserの責務。Rust u64引数にboolが来ると仮定した試験は書かない。
- 実I/O alignment、pinning、kernel pointer、memory allocationとは独立。

## 3. 固定契約: in-memory origin（F04）

```rust
pub enum OriginError { CapacityTooLarge, Range(RangeError), UnknownRange }
pub struct MemoryOrigin; // 内部はprivate
impl MemoryOrigin {
    pub fn new(initial: Vec<u8>) -> Result<Self, OriginError>;
    pub fn read(&self, offset: u64, length: u64) -> Result<Vec<u8>, OriginError>;
    pub fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), OriginError>;
    pub fn discard(&mut self, offset: u64, length: u64) -> Result<(), OriginError>;
}
```

最大容量1,048,576 bytes。初期bytesは既知。不正rangeのwrite/discardでは状態を変更しない。discard範囲はunknownにし、unknownを含むreadはUnknownRange。writeした範囲だけknownへ戻す。zero lengthは成功。全てin-memory、実ファイル・device・thread・時間待機なし。

この型単独では並行性・flush・電源断耐久性を表現しない。D02/D07が定義する実行/永続化モデルと混同しない。

## 4. 固定契約: latency統計（B01）

```rust
pub struct LatencySummary {
    pub count: usize, pub minimum_ns: u64, pub p50_ns: u64,
    pub p95_ns: u64, pub p99_ns: u64, pub maximum_ns: u64,
}
pub enum StatsError { Empty }
pub fn summarize_ns(samples: &[u64]) -> Result<LatencySummary, StatsError>;
```

nearest-rank方式。昇順配列の1-based ceil(p*n)番目を採用、入力を変更しない。index計算はusizeの乗算overflowを避ける。[1,2,3,4,100] → p50=3,p95=100,p99=100。1要素は全分位点が同値。外れ値除去・欠測補完・統計的有意性の主張をしない。

## 5. 固定契約: label manifest（F02）

`.github/labels.json`: `schema_version=1`、`labels` は `{name,description}` の配列。nameはASCII、1〜50 bytes、同名禁止。descriptionは1〜100 Unicode scalar values。許可prefixは `agent:`, `review:`, `status:`, `priority:`, `area:`, `type:`, `decision:`。未知キー、未知prefix、重複JSONキーを拒否。

```rust
pub fn validate_label_manifest(input: &str) -> Result<(), Vec<ManifestViolation>>;
```

ManifestViolationは `{ code: String, path: String }`。codeはINVALID_JSON/UNKNOWN_FIELD/UNSUPPORTED_SCHEMA/DUPLICATE_NAME/INVALID_NAME/INVALID_DESCRIPTION/UNKNOWN_PREFIX。pathはJSON Pointer。syntax errorのpathは空文字。複数違反は(code,path)昇順で重複除去。parserが単一の構造errorで停止してよいが、parse済みmanifestの全ラベルのsemantic errorは集約する。**GitHub APIへの書込み・ラベル自動削除はしない。** 色はこのmanifestで管理しない。

## 6. 固定契約: offline status v1（C01/C02/C03）

これはGUI/CLIの表示テスト用schemaであり、kernel IOCTLのABIではない。

```json
{
  "schema_version": 1,
  "state": "disabled",
  "mode": "read_cache_wi",
  "counters": {
    "read_hits": null, "read_misses": null,
    "origin_read_bytes": null, "cache_written_bytes": null
  },
  "error_code": null
}
```

- state: disabled/validating/shadow/active/quiescing/degraded/removing。
- modeはread_cache_wiのみ。writebackに切り替えるfieldを作らない。
- counterはnull（未取得）またはu64範囲の10進文字列。0は `"0"`、正数は先頭0なし。JavaScript Numberに変換して精度を失わない。
- error_code: null またはCACHE_IO/CACHE_CORRUPT/TARGET_CHANGED/UNSUPPORTED/RESOURCE_LIMIT/QUIESCE_PENDING。未知文字列をログへそのまま流さない。
- 全必須キー、未知キー、重複キー、未対応schema、型違いを拒否。入力上限65,536 UTF-8 bytes。
- `parse_status(input: &str) -> Result<StatusSnapshot, StatusError>`
- `render_status(snapshot: &StatusSnapshot, format: OutputFormat) -> String`。OutputFormat=Json/Text。
- status activeは「安全性証明済み」の表示ではない。state名、mode、counter未取得/値、固定error説明だけを表示。
- `cache-cli status --input <file> --format json|text` はofflineのみ。成功0、input/schema error2、file I/O error3。
- `public_diagnostics(input: &str) -> Result<PublicDiagnostics, StatusError>` はstrict parse済みstatusのallowlistだけを出力。paths、serial、ユーザー名、file content、raw error messageを追加しない。不正入力のerror表示にも元payloadを含めない。
- C04でlive serviceと統合する際、D06が両者のschema対応をレビューする。offline schemaを都合よく特権wire protocolへ転用しない。

## 7. 固定契約: benchmark v1（B01/B02）

JSON objectの全必須field:

- schema_version=1
- workload_id, environment_id, toolchain_id: 1〜64 ASCII文字、[A-Za-z0-9._-]のみ。environment_idは匿名ラベル。
- working_set_bytes: u64の非zero 10進文字列
- os_cache: cold/warm/unknown
- cache_mode: off/shadow/active
- ssd_cache: none/cold/warm
- duration_ns: 非zero u64 10進文字列
- latency_ns: 1〜100,000個のu64 10進文字列（0可）
- cpu_time_ns、peak_nonpaged_bytes、ssd_written_bytes: nullまたはu64 10進文字列
- repetitions: integer 1〜1,000

unknown field/duplicate key/oversized input（16 MiB超）を拒否。比較はworkload_id/environment_id/toolchain_id/working_set_bytes/os_cache/repetitionsが一致し、os_cacheがunknownでない場合だけ許可。cache_mode/ssd_cacheは比較する処置条件として必ず両方表示する。offはssd_cache=noneのみ、active/shadowはcoldまたはwarm。mode差を隠さない。

`compare_runs(base: &BenchmarkRun, candidate: &BenchmarkRun) -> Result<BenchmarkComparison, CompareError>`。比較不可理由はfield名だけの固定列挙。speedup=base.duration/candidate.duration、roundingは表示だけ。1.0秒→0.8秒は1.25倍。機器条件を偽らないためenvironment_idの元情報は実験台帳で別管理する。p95/p99を報告し、SSD hit率から架空のtask速度を計算しない。

## 8. 設計ゲートで凍結する契約

ここを未決のままLunaに実装させない。

| 契約 | owner | 必須出力 | 後続 |
|---|---|---|---|
| trace schema / linearization | D02 | event enum、必須field、request/epoch/slot ID、上限、全正常/異常例 | F05,F06,F07 |
| durability/fault oracle | D07 | accepted/completed/flushed writes、TRIM unknown、seed replay、mutation-test反例 | F06,F08,Q01 |
| engine/FFI | D03,D04 | opaque handles、repr(C)固定幅型、error/result、buffer owner、callback IRQL、allocator | W02 |
| target descriptor | D05 | 三値属性yes/no/unknown、identity、geometry、拒否code、TOCTOU再検証 | C05 |
| IOCTL / service IPC | D06 | byte layout/version/length/opcode、ACL/authz、rate/size limits、golden vectors | C04,C06,U01 |
| resource budgets | D04,B03 | max inflight/queue/cache size/nonpaged memory、overflow/low-memory動作 | W03,W07 |

D02がtraceを凍結する際、origin-write submit/apply/complete、read snapshot/completion、cache-fill submit/complete/publish、cancel request、epoch change、slot reuse、flush、discardを表現できること。cancelとcomplete、cache data writeとmetadata publishを別eventにする。最低反例: stale-fill、2 writers、partial overlap、late DMA、late physical slot write。容量上限1 MiB、event数上限10,000、payload総量16 MiB。seedはtraceに記録し、OS時刻やthread schedulerに依存しない。

## 9. Rust / FFI境界

- unsafe/Send/Syncの各実装にSAFETY根拠、owner、aliasing、IRQL、completion後解放条件を書く。
- Pin/Arc/Dropはdevice I/O完了の代わりにならない。非同期callbackが終了する前に参照・buffer・SSD slotを再利用しない。
- panic/unwindをFFI/kernel境界に越境させない。panic/OOM方針はD03/D04で定義し、panic=abortだけを安全な回復策と呼ばない。
- Rust参照/Vec/String/未検証enumをC/wireへ直接渡さない。固定幅値・checked length・opaque handleを使い、invalid valueをRustの有効な型へ作り込まない。
- host向け依存をno_std kernelへ流用しない。allocator/Dropが待機できるIRQLとpoolを明示。
- 生のdisk操作をTauri commandとして公開しない。frontend validationとは別にserviceで再検証する。


## 10. 固定契約: fake completion queue（F03）

```rust
pub struct RequestToken; // private monotonically increasing u64、Copy/Eq/Ord
pub struct Completion {
    pub token: RequestToken,
    pub payload: std::sync::Arc<[u8]>,
    pub cancel_requested: bool,
}
pub enum QueueError {
    Full, PayloadTooLarge, PayloadBudgetExceeded, UnknownToken, TokenExhausted
}
pub struct CompletionQueue;
impl CompletionQueue {
    pub fn new() -> Self;
    pub fn reserve(&mut self, payload: std::sync::Arc<[u8]>)
        -> Result<RequestToken, QueueError>;
    pub fn request_cancel(&mut self, token: RequestToken) -> Result<(), QueueError>;
    pub fn deliver(&mut self, token: RequestToken) -> Result<Completion, QueueError>;
}
```

pending最大1,024件、1payload最大1,048,576 bytes、queueが保持するpayload length合計最大16,777,216 bytes。reserveの拒否優先はFull→PayloadTooLarge→PayloadBudgetExceeded→TokenExhausted。失敗で既存queueを変えない。tokenは1から単調増加し再利用しない。上限到達時はTokenExhausted、wrapしない。

cancelはpending flagだけを立て、pending中は冪等成功。完了や解放を意味しない。deliverは明示呼出しだけで起こり、順序を自由に変えられる。二重deliver/完了後cancel/未知tokenはUnknownToken。deliver後payloadは返却Arcの所有により有効。これはhost-onlyのモデルであり、device I/Oや実DMA停止を模擬できたという意味ではない。

## 11. バックログのmachine-readable契約（F02）

`docs/backlog.json`: schema_version=1、plan_date（YYYY-MM-DD）、tasks配列。各taskの必須キーはid/title/agent/review/status/priority/type/area/owner_decision/dependencies/files/output/acceptance/escalation/issue_number/issue_url/completion。

- idはGOV01、D01等の大文字英数stable ID、一意。
- agent=luna/sol/astra、review=sol/astra、status=ready/blocked/needs-decision、priority=p0/p1/p2、type=design/task/gate、areaはlabels.jsonに定義したarea suffix。
- owner_decisionはbool。dependenciesは他task IDの重複なし配列、DAGであること。self-reference禁止。
- issue_numberは正整数、issue_urlはこのrepoの対応URL。初回起票中のみnullを許容するが、完成したbaselineではnullなし。
- completion=pending/accepted/not-planned。初回は全件pending。acceptedへ変える際はIssueのreview/test証拠へのリンクを本文/計画へ残す。
- readyは全dependenciesがacceptedで、owner_decision=falseの時だけ許可。needs-decisionはowner_decision=true。blockedは未解決前提を本文に示す。
- filesは予定path配列であり、codeが現存するという判定には使わない。相対Markdownリンクは現存docのみを解決し、code span内のplanned pathは欠落と誤認しない。
- これは計画時点のsnapshot。GitHubのlive進捗と自動同期されるとは主張しない。契約変更/依存変更/受入完了時にsnapshotもPRで更新する。
