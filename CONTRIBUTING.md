# 貢献ガイド

現在は設計段階です。ライセンス/外部contribution条件の採択前は、外部コードの取り込みや一般向け公開を開始しません。GOV01の決定を待ってください。

1. [設計](docs/README.md)と[担当ルール](docs/agent-routing.md)を読む。
2. [Issue一覧](docs/backlog.md)からstatus:readyを選び、依存と必要reviewを確認する。
3. 一つの受入単位をbranch/PRにまとめる。Rustを主とし、必要なGUIはTauri、Windows固有の言語例外はADRにする。
4. テスト先行で実装し、再現コマンド・実行環境・未実行試験をPRへ記録する。
5. 作者と別のreviewerが確認。安全契約に触れる場合はreview:astra以上、出荷前は人間の専門レビューも必要。
6. merge後にIssueへ証拠を残し、依存課題のready条件を再評価する。

planned command: `cargo test -p cache-types` 等。現在まだCargo workspaceはありません。F01が作成するまで、実行可能と誤認しないでください。

kernel/実機試験は開発専用環境でのみ行います。Issueに実装を依頼されただけで、利用者の普段のHDDを初期化する権限は生じません。
