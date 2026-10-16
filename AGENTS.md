# EveryDB 開発ルール

本ファイルは EveryDB 固有の必須ルール、配置、資料目録の正本です。現在の作業に必要な資料だけを読み、既存のユーザー変更と製品固有の構成を保持してください。

## 必須ルール

- PowerShell は PowerShell 7 以降の `pwsh` を使用する。
- ドキュメントとコミットログは日本語で記録する。
- 秘密値をソース、文書、ログ、コミットへ残さない。公開可能な証明書メタデータと秘密鍵を区別する。
- 大きな変更はフェーズとチェックリストへ分ける。各フェーズで関連テストとビルドを実行し、成功後に日本語のコミットを作る。
- 外部公開、署名、既存リリースの置換は、ユーザーが明示的に依頼した場合だけ行う。
- 過去資料の未チェック欄だけで未実装と判断せず、後続の完了記録とユーザー確認を照合する。

## 構成と配置

| 配置 | 役割 |
| :--- | :--- |
| `README.md` | 利用者向けの入口 |
| `docs/` | 利用者および公開開発者向けの説明、仕様、再現手順 |
| `dev/` | 開発計画、判断、作業・検証記録 |
| `dev/scripts/` | ビルド、起動、検証、配布の運用スクリプト |
| `dev/roadmap/` | 将来案と不採用判断 |
| `dev/testing/` | 継続更新する検証記録 |
| `dev/private/` | 秘密情報・個人情報を含む非公開資料。Git・公開成果物・通常調査から除外 |
| `dev/temp/` | プロジェクト外の一時制作物。Git・目録・通常調査から除外 |
| `dev/vX.Y.Z-req.md`、`-imp.md`、`-task.md` | 実装予定・実装中の版別資料 |
| `dev/history/vX.Y.Z/` | 完成済みの版別資料一式 |
| `src/` | TypeScript フロントエンドと固有テスト |
| `src-tauri/` | Rust／Tauri ソース、製品同梱リソース、Rust 固有テスト |
| `shared/` | TypeScript と Rust が共有する宣言的データ |
| `tests/` | 複数構成から使う共通試験・fixtures・期待結果。必要になった時だけ作る |
| `release/` | 配布成果物。Git 対象外 |

ツール固有の配置例外として、`scripts/*.mjs` は npm の品質ゲート、`src-tauri/examples/` は Cargo の開発用 example、`.github/workflows/` は CI 定義に使用する。`src-tauri/examples/` が生成するローカル試験データは `samples/` に置き、Git 対象外とする。製品同梱サンプルは `src-tauri/resources/` に置く。

取り込み用の `templates/` または `dev/templates/` はリポジトリに保持しない。製品機能またはツール固有の同名フォルダが必要になった場合は、用途を本節へ追記する。

## 資料を読む条件と目録

| 作業・目的 | 読む資料 |
| :--- | :--- |
| 概要・利用方法 | [README.md](README.md)、[使い方](docs/user-guide.md)、必要な接続先ガイド |
| ビルド・改造 | [開発者ガイド](docs/DEVELOPERS_GUIDE.md)、[ビルド方法](docs/development/building.md)、[テスト方法](docs/development/testing.md) |
| 要求の実装 | 対象の `dev/vX.Y.Z-req.md` と対応する `imp`・`task` |
| API・内部仕様 | [アーキテクチャ](docs/development/architecture.md)、該当する `docs/` と対象ソース |
| データソース追加 | [データソース追加手順](docs/development/adding-a-data-source.md)、該当ガイドと registry |
| 将来機能 | [将来案](dev/roadmap/PLAN_IDEAS.md)、必要な不採用記録 |
| 配布・公開 | [配布手順](dev/RELEASE_RULE.md) |
| 開発資料の所在 | [dev 目録](dev/README.md) |
| 過去の判断・結果 | `dev/history/` の必要な版と、その後の完了記録 |

`dev/private/` と `dev/temp/` の内容は、ユーザーが対象を明示した場合だけ読む。存在しない例示資料や全資料の一括読込みを前提にしない。

## 版別資料の運用

- 要求は `dev/vX.Y.Z-req.md`、AI の実装計画・報告は `dev/vX.Y.Z-imp.md`、要求が粗い場合の進捗は `dev/vX.Y.Z-task.md` に記録する。
- 実装予定・実装中の複数版は `dev/` 直下へ置ける。完成した一式だけを `dev/history/vX.Y.Z/` へ移す。
- 未完了事項を別版や roadmap へ送る場合、移動元と引き継ぎ先の双方に出典と参照を残す。次版開始やリリースだけを完了理由にしない。

## スクリプト

製品操作用の入口は `動詞-モード-EveryDB-機能[-検証データ].ps1` とする。動詞は `build`、`run`、`test`、`publish`、モードは `debug`、`release`、`exe` を使用し、アプリ全体は `all` とする。

- `build`: ビルドのみ。起動、テスト、インストーラ生成、公開を含めない。
- `run`: debug／release はビルド後に起動し、exe は既存成果物だけを起動する。
- `test`: 固定した入力・期待結果で自動判定し、成功時0、失敗時非0を返す。
- `publish`: 配布物生成と規定の外部公開を行う。外部公開の明示依頼が必要。

モードに依存しない補助処理は `動詞-対象または目的.ps1` とし、目的を [dev 目録](dev/README.md) に記載する。既存入口を改名したときは、呼び出し元、文書、CI、テストを同時に更新する。

## Git と配布

`dev/` の正式資料、運用スクリプト、試験は追跡する。`dev/private/`、`dev/temp/`、`release/`、`*.pfx`、`*.p12` は追跡しない。既に秘密情報をコミットしていた場合、値を転載せず、失効・再発行と履歴除去の要否を報告する。

配布物生成と外部公開は分ける。実在する手順、成果物、署名方針は [dev/RELEASE_RULE.md](dev/RELEASE_RULE.md) を正本とする。

## コミット

個人開発では `main` を基本とし、大規模・長期変更だけ機能ブランチを使う。コミットは `feat:`、`fix:`、`docs:`、`style:`、`refactor:`、`test:`、`chore:`、`release:` などの簡略 Conventional Commits と日本語要約を使用する。
