# アーキテクチャ

- `src/app/`: 画面初期化、IPC契約、アプリ状態。
- `src/features/`: 検索、グリッド、ソース編集などの機能別UI。
- `src/shared/`: 型、ストレージ、URL、Excel列選択などの副作用を持たない共通処理。
- `src/source-types/registry.ts`: データソースのカテゴリ、親種別、credentialスコープ、手順書の共通metadata。
- `src-tauri/src/*_commands.rs`: Tauri IPCコマンド。
- `src-tauri/src/app_state.rs`: DB接続、Windows Hello期限、OAuth取消状態、スケジューラ終了状態。
- `src-tauri/src/refresh_service.rs`: 設定取得、ロック外の外部取得、結果保存を分離した再取得処理。
- `src-tauri/src/opener.rs`: 外部参照を開く安全ポリシー。
- `src-tauri/src/connectors/`: 各データソースのコネクタ実装とfactory。
- `src-tauri/src/db/`: SQLiteスキーマ、migration、repository処理。

依存方向は、UI → IPC → service/repository/connector を目標とします。connectorの外部I/O中はSQLiteロックを保持しません。
