# win-ssd-hdd-cache

Windows向けに、既存のHDDを正本に保ち、SSDを破棄可能な読み取りキャッシュとして利用するOSSを設計・開発するプロジェクトです。

**現在は設計とportable Rust基盤の実装段階です。実用ドライバー、インストーラー、対応を保証するWindowsビルドはまだありません。実データを接続して試用できる状態ではありません。**

## 方針

- 最初はデータ用HDDと別SSDに限定した **clean read cache / Write-Invalidate**。書き込みの正本は常にHDDです。
- SSDキャッシュの消失時も、原本の読み書きの意味を変えない設計を目指します。ただし「read cacheだから無条件に安全」とは扱いません。
- 初期版ではwrite-back、起動ディスク、ページファイル、再起動を跨ぐキャッシュ再利用は扱いません。
- WindowsのI/O捕捉位置とOCF採否は、調査・実証・レビューを経て決めます。既存Linux実装をそのままWindowsに持ち込める前提にはしません。
- **主要実装はRust、GUIが必要な部分はTauri**。Windowsの核心部分だけ、必要に応じC/WDK等を局所採用します。
- 小さな実装課題と、高度な設計・独立レビューが必要な課題を分離します。

## 開発状況

portable host基盤・整合性契約・strict trace parser・独立oracle仕様・Rust engine feasibility・Rust/WDK build-only proofまで、**16課題を独立受入してmainへ統合**しました。これは製品driverの完成ではありません。Windows/Linuxのhost CIと、WDKのcompile/link・C ABI検証は実行済みですが、driver load・実機安全性・性能の検証は未実施です。[最新の統合記録](docs/evidence/2026-11-03-phase2-integration.md)、[開発手順](docs/development.md)、[全課題](docs/backlog.md)を参照してください。

[ロードマップ #1](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/1)・[設計文書](docs/README.md)・[貢献ルール](CONTRIBUTING.md)。This is portable-host research and implementation, not a production driver.

## 次に進める主要Issue

- [F06 #20: deterministic replay / independent oracle](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/20) — 前提のF05/D07は受入・main統合済み。実装・故障model検出はこれから。
- F07/F08/F09はF06等の後続。古いcache公開・早期ACK・late I/Oの検証を実装した上で次のgateを判定します。
- [D01 #3: I/O捕捉位置](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3) — 調査文書とlab手順は統合済み。位置は未採択で、別承認の使い捨てWindows labの実trace待ち。
- [全Issueと依存関係](docs/backlog.md) / [モデル担当とreview規則](docs/agent-routing.md)。

GUIはTauriを用いますが、CLI/serviceと安全なdata pathの契約を先に固めます。

## ライセンスと公開

リポジトリは所有者の判断でpublicになりました。通常のGitHub Actionsによる検証も許可されています。ただし、ソースの閲覧可能性とOSSライセンスの採択・外部contributionの受付・製品の公開/配布は別です。ライセンスや配布条件はGOV01の所有者決定を待ちます。通常のCargo/npmパッケージ依存はlicense・hashを確認して利用できます。第三者ソースのvendoringや外部contributionの受付はGOV01の方針採択後とし、通知を保持します。現時点でライセンスが付与済みの完成OSSであるとは表示しません。

