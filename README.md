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

portable Rust基盤・strict codec・offline CLI・計画checker・host CIの10課題と監査課題AUD01をPR #54〜#65でmainへ統合しました。さらにD02の整合性契約・有限状態checkerを独立レビュー後にPR #67で統合しました。D02 CIはLinux 102件、Windows Server 100件のhost試験に成功しています。これは製品driver・実機安全性の受入ではありません。[最新の統合・検証記録](docs/evidence/2026-11-02-main-integration.md)、[統合前の試験履歴](docs/evidence/2026-11-02-host-foundation.md)、[開発手順](docs/development.md)、[バックログ](docs/backlog.md)を確認してください。

[ロードマップと課題一覧](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/1) を起点にしてください。[正式設計と関連文書](docs/README.md)・[貢献ルール](CONTRIBUTING.md)を参照してください。

This project is in the **design and portable-host implementation phase**: a discardable, clean SSD read cache for selected Windows data HDDs. No production driver or verified hardware support is available yet.

## 次に進める主要Issue

- [F05 #14: strict trace parser](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/14) / [D07 #17: 独立oracle契約](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/17) — 受入済みD02を使うportable検証。
- [D03 #8: cache engine比較](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8) / [D09 #6: Rust/WDK境界](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6) — host実証・build-only proofと独立レビュー。
- [D01 #3: I/O捕捉位置](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3) — 調査文書とlab手順は統合済み。位置は未採択で、別承認の使い捨てWindows labの実trace待ち。
- [全Issueと依存関係](docs/backlog.md) / [モデル担当とreview規則](docs/agent-routing.md)。

GUIはTauriを用いますが、CLI/serviceと安全なdata pathの契約を先に固めます。

## ライセンスと公開

リポジトリは所有者の判断でpublicになりました。通常のGitHub Actionsによる検証も許可されています。ただし、ソースの閲覧可能性とOSSライセンスの採択・外部contributionの受付・製品の公開/配布は別です。ライセンスや配布条件はGOV01の所有者決定を待ちます。通常のCargo/npmパッケージ依存はlicense・hashを確認して利用できます。第三者ソースのvendoringや外部contributionの受付はGOV01の方針採択後とし、通知を保持します。現時点でライセンスが付与済みの完成OSSであるとは表示しません。

