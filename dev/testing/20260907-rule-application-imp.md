# 2026-09-06 共通開発ルール適用計画・報告書

参照指示: `20260906版ルール適用指示書.md`

作成日: 2026-09-07

状態: 完了

## 0. 事前確認

- 適用前の作業ツリーはクリーンで、既存のユーザー変更はなかった。
- ルート `AGENTS.md`、`dev/README.md`、`dev/RELEASE_RULE.md` は未作成だった。
- `.gitignore` は `dev/*` を原則除外する方式で、正式資料を個別に再許可していた。
- v0.1.0 は要求・実装・タスク資料とも完了扱いだが、`dev/` 直下に残っていた。
- `dev/history/` の旧版資料は版別ディレクトリではなく直下に置かれていた。
- `dev/templates/` に取り込み用テンプレートが残っていた。
- 製品同梱サンプル `samples/customers.csv` は、ローカル生成データと同じ除外ディレクトリにあり、`samples/` では12ファイルが「Git追跡済みかつ除外対象」になっていた。
- 旧運用スクリプト4本は、名称から動詞・モード・対象を一意に判断できなかった。

質問を要する仕様選択はなく、既存の製品・配布方針を維持して配置と入口を整理する。

## 1. 実装順序とコミット単位

| # | 内容 | 対象 |
| :--- | :--- | :--- |
| 1 | ルール正本、目的別目録、公開開発者ガイド、配布手順を整備 | `AGENTS.md`、`README.md`、`docs/`、`dev/` |
| 2 | 完了済み版別資料、将来計画、取り込み用テンプレートを所定位置へ整理 | `dev/history/`、`dev/roadmap/`、`dev/templates/` |
| 3 | サンプルと運用スクリプトを用途・実処理に合う配置と名称へ変更 | `src-tauri/resources/`、`dev/scripts/`、参照元 |
| 4 | 文書、PowerShell、Git除外、品質ゲート、ビルドを検証して本書を完成 | 検証一式 |

変更は共通ルール適用という密結合した一単位として、全検証成功後に1コミットへまとめる。

## 2. 実装方針

### 2.1 ルールと資料分類

- default 原本の共通事項を EveryDB の実在構成へ合わせ、ルート `AGENTS.md` を正本にする。
- `docs/DEVELOPERS_GUIDE.md` を公開開発文書の入口、`dev/README.md` を内部作業資料の入口にする。
- `dev/RELEASE_RULE.md` は Tauri の MSI／NSIS 生成と GitHub Releases 公開を明確に分離する。

### 2.2 版別資料と取り込み資料

- 完了済み資料は `dev/history/vX.Y.Z/` へ版ごとに移す。
- 将来案と不採用案は `dev/roadmap/` へ移し、参照を修正する。
- `dev/templates/` は正式資料への移管後に削除する。

### 2.3 Git除外とサンプル

- `dev/private/`、`dev/temp/`、`release/`、秘密鍵付き証明書を明示除外し、正式な `dev/` 資料は追跡可能にする。
- 製品同梱 `customers.csv` は `src-tauri/resources/samples/` に置く。ローカル生成試験データ用の `samples/` は除外を維持する。

### 2.4 スクリプト

- デバッグ通常起動、デバッグサンプル起動、Releaseビルド後起動、既存exe起動を別入口にする。
- 補助生成スクリプトはモードを持たない補助命名へ変更する。
- 実行ファイル起動ではビルドへ暗黙フォールバックしない。

## 3. 影響範囲

文書、Git除外、Tauriリソース指定、PowerShell運用スクリプト、文書検査スクリプト。製品ロジックや外部公開処理は変更しない。

## 4. 実装で判明した追加対応

- `.gitignore` の `dev/*` 除外を廃止したことで、従来は未追跡のまま隠れていた接続情報設定シート、Microsoft 365 Entra user 関連資料、`secrets/` が見つかった。内容は読み込まず、3系統を保持したまま `dev/private/` へ移管した。
- `samples/` の追跡済み12ファイルはローカルに保持したまま追跡だけを解除した。製品で必要な `customers.csv` は同内容の正本を `src-tauri/resources/samples/` へ移管した。
- 旧履歴には、後年のソース再編で現存しないコードへの行リンクが含まれる。旧名称と当時の参照先は歴史的記録として保持し、文書間リンクだけを検査・修正するよう文書検査を調整した。
- サンドボックス内の `check:all` では Windows DPAPI の `CryptProtectData` が利用できず、Rust 37件が環境要因で失敗した。同じテストと完全な品質ゲートを通常のWindowsユーザー環境で再実行すると全件成功した。
- Tauri Release ビルドは既存 identifier `com.everydb.app` が `.app` で終わる点をmacOS向け注意として警告した。EveryDB の対象環境は Windows であり、ビルドは成功したため本作業では識別子を変更していない。

## 5. 進捗

- [x] ルール・目録・公開文書の整備
- [x] 版別資料・テンプレートの整理
- [x] サンプル・スクリプトの整理
- [x] 検証

### コミット

| コミット | 内容 |
| :--- | :--- |
| `docs: 共通開発ルールをEveryDBへ適用` | ルール正本、資料配置、Git除外、サンプル、運用スクリプト、検証の一式 |

## 6. 検証結果

| 対象 | 結果 |
| :--- | :--- |
| Markdownリンク・旧名称 | `npm run docs:check` 成功、52ファイル検査 |
| スクリプト命名 | `test-script-naming.ps1` 成功 |
| PowerShell構文 | `dev/scripts/*.ps1` 全6本で Parser エラー0件 |
| Git除外 | `dev/private/`、`dev/temp/`、`release/`、`*.pfx`、`*.p12` の除外元を確認。追跡済み除外ファイル0件 |
| 製品同梱サンプル | 移管元と `src-tauri/resources/samples/customers.csv` の SHA-256 が一致 |
| 全品質ゲート | 通常のWindowsユーザー環境で `npm run check:all` 成功 |
| フロントテスト | 40ファイル、151件成功 |
| Rustテスト | 207件成功、16件は外部サービス・実環境を要するため既定どおり ignored |
| Rust静的検査 | `cargo fmt --check`、Clippy（warnings禁止）成功 |
| ビルド | フロント本番ビルド、Rust debug bin、`npm run tauri build -- --no-bundle` の Release ビルド成功 |
| 差分 | `git diff --check` 成功 |

外部公開、署名、MSI／NSIS生成、ライブ接続テスト、アプリ実機操作は本指示の検証範囲に含めず実施していない。

## 7. 残件・申し送り

- Tauri identifier のmacOS向け警告は既存設定に由来する。Windows専用方針を変更する場合に別途判断する。
- `samples/` のローカル試験データには大容量ファイルが含まれるため、実ファイルを削除せずGit追跡だけを解除した。製品同梱サンプルの正本は `src-tauri/resources/samples/customers.csv` とする。
- `dev/private/` へ移した3系統は Git 履歴には存在せず、ローカルで保持している。
