# セキュリティ、署名、配布、運用

状態: 設計。証明書購入、アカウント登録、公開、実機設定変更は本計画作成の権限に含めない。

## 1. 脅威と境界

主な資産は原本HDDの内容、kernel memory、管理権限、cacheに残る内容、署名鍵、更新の信頼性。

| 境界 | 危険 | 対策 |
|---|---|---|
| 一般user → service | raw disk操作や他user内容への権限昇格 | ACL付きlocal IPC、peer認証/認可、操作別権限、server側再検証 |
| service → kernel | malformed IOCTL、任意pointer/len、任意disk | versioned bounded ABI、allowlist opcode、checked ranges、独立review |
| Rust → C/WDK/OCF | aliasing、invalid enum、panic unwind、二重解放 | narrow FFI、opaque handle、固定幅型、SAFETY契約、callback IRQL |
| GUI → service | XSSから管理操作 | Tauri capability/CSP最小化、任意shell/raw IOCTL禁止、非特権UI |
| SSD → kernel/caller | 不正/古いcache、hang、late DMA | 検証/epoch/range/slot ownership、独立buffer、原本fallback |
| 開発者 → release | PRが署名鍵へ到達、依存差替え | trusted release分離、immutable pin、review、SBOM/通知 |
| 運用 → confidential data | cache残留、診断漏洩 | allowlist診断、暗号化境界検証、消去の限界を明示 |

local administrator/kernel侵害まで防げると主張しない。一方、管理者だけが設定する製品だから無権限のIOCTLを安全確認不要とはしない。

## 2. Tauriの制約

TauriはGUI層だけ。status表示と、固定された管理APIへのUIを提供する。WebViewを管理者権限で常用しない。remote content、任意filesystem/shell/network権限を既定で与えない。CSP/capabilitiesをversion管理し、Vue/TypeScriptの型とRust/serviceの検証を独立に保つ。

破壊的なcache partition初期化は、UIの確認ボタンだけでは不十分。serviceが現在のtarget fingerprint、対象がcache専用で原本/OSではないこと、明示確認tokenを再検証する。GUI導入で既存CLIの安全条件を緩めない。live操作はC04/C06完成後、U01で権限レビューしてから。

## 3. 暗号化と残留データ

filterが暗号化層のどちら側を見るかでcacheの機密性が変わる。初期版でBitLocker/device encryption対象を非対応にする判断も、正確な検出とサポート境界が必要。D01/D05/D06で検証し、不明な構成を許可しない。

EFS等のfile-level暗号化については、FS下層で何が見えるかの証拠をD06で確認する。単に「SSDは別diskだから安全」としない。cacheには削除済み内容が残り得る。TRIM、format、entry invalidateを完全消去の保証として説明しない。cache encryptionは別RFCで扱い、本初期計画で未実装の保護を宣伝しない。

## 4. 署名・認定・配布を区別

Microsoftの一次資料を2026-11-02に確認した時点:

- EV certificateだけでは一般利用者向けkernel driverの信頼条件を全て満たさない。Hardware Developer Program/Partner Centerの条件とMicrosoft署名の経路を確認する。
- HLK/WHCP経由の署名、attestationのテスト用途、preproductionのprovisioned test device、retail Windows Update公開は別の条件。
- 現行のdriver-signing-offeringsはattestationをtesting purposes onlyとし、retail Windows Update公開は不可としている。
- attestation署名されたbinaryが技術的にload可能なことと、製品配布の条件を満たすことは区別する。一般公開の計画はWHCP/HLKを基本とし、別経路を採るならその時点の条件を確認する。
- HVCI/Secure Bootを一般利用者に無効化させることを製品導入手順にしない。test signingは隔離された開発機だけ。
- Hardware Developer ProgramのEV/組織情報/本人・事業情報等の要件はD08で最新確認。個人名義やsole proprietorでの可否を推測で断定しない。

[資料一覧](references.md)を参照。費用はcertificate、署名主体、lab機器、専門家review、保守に分けて見積り、見積りと購入承認を分離する。所有者承認前に購入/登録/署名鍵投入を行わない。

## 5. Release工程

1. source/tag/dependency hashes、license/third-party notices、SBOMを確定。
2. host tests、driver build、対象matrixの実機証拠、rollbackをreview。
3. trusted isolated jobで署名。鍵をrepo、PR job、診断artifactへ入れない。
4. package hash、対応範囲、既知非対応、data-loss warning、撤去/復旧手順を付ける。
5. staged/opt-in配布。kernel driverの無確認auto-updateを初期版に入れない。
6. update失敗時はcache無効状態を優先し、originを変えず、必要なrebootを明示。
7. rollbackで古いpersistent cacheを再採用しない。全sessionをcold reset。

署名したという理由で安全性未確認のbetaをstableと名付けない。

## 6. OSSガバナンス

GOV01でライセンスを明示決定する。提案はRustの慣例との親和性を考えMIT OR Apache-2.0（original code）だが、採択前にLICENSEを作成して権利付与済みとしない。OCF等の第三者コードは各license/noticeを保持。外部contributionの受付条件（DCO/CLA要否）も決める。

私有repoを自動公開しない。公開時は、security reporting経路、maintainer、支援可能なWindows build、issue template、release policyを整える。脆弱性報告経路が未設定の間は、個人dataを含むdumpをpublic issueへ投稿させない。

## 7. 運用停止条件

データ差異、use-after-free、二重completion、未知mutationの監視漏れ、cache/原本の対象誤認、未検証の暗号化境界が判明したら、active利用を停止し、再現traceと証拠を残す。再開はAstra級reviewだけでなく、出荷段階では人間の専門家が判断する。
