# アーキテクチャ

- `src/main.ts`: 現在の画面初期化と既存UI機能。段階的にfeature単位へ分割中。
- `src/shared/`: 型、ストレージ、URL、Excel列選択などの副作用を持たない共通処理。
- `src/source-types/registry.ts`: データソースのカテゴリ、親種別、credentialスコープ、手順書の共通metadata。
- `src-tauri/src/commands.rs`: Tauri IPCコマンド。
- `src-tauri/src/app_state.rs`: DB接続、Windows Hello期限、Box OAuth取消状態。
- `src-tauri/src/refresh.rs`: 設定取得、ロック外の外部取得、結果保存を分離した再取得処理。
- `src-tauri/src/opener.rs`: 外部参照を開く安全ポリシー。
- `src-tauri/src/connectors.rs`: 各データソースのコネクタ実装。段階的にサブモジュールへ分割予定。
- `src-tauri/src/db.rs`: SQLiteスキーマとリポジトリ処理。段階的にmigration/repositoryへ分割予定。

依存方向は、UI → IPC → service/repository/connector を目標とします。connectorの外部I/O中はSQLiteロックを保持しません。
