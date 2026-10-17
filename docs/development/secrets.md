# 開発用秘密情報の保管

開発用のAPIトークン、クライアントシークレット、接続情報は、リポジトリの外にある `C:\AI\env\EveryDB\` へ保存します。Git管理・配布物・版別ログへ秘密値を含めません。

## 初回準備

```powershell
New-Item -ItemType Directory -Path C:\AI\env\EveryDB -Force
if (-not (Test-Path C:\AI\env\EveryDB\development.env)) {
    Copy-Item config/development.env.example C:\AI\env\EveryDB\development.env
}
```

既にファイルがある場合は上書きせず、そのファイルを編集します。値を入力するときはエディターを使用し、秘密値をコマンド履歴へ残さないようにします。Googleの資格情報JSON、秘密鍵、接続情報のメモも同フォルダーへ置き、環境変数にはファイルパスを指定します。

`development.env` は `EVERYDB_変数名=値` の形式です。空欄と `#` から始まるコメント行を使えます。値の外側の単一・二重引用符は除去し、変数展開やPowerShell式の評価は行いません。重複キー・形式不正は全体を拒否し、値をエラーへ出力しません。

## 読込み

開発用の `run-debug`、`run-release`、`run-exe` の各起動スクリプトは、このファイルが存在すれば自動で読み込みます。既に設定されているプロセス環境変数を優先します。読み込むのはそのプロセスだけで、ユーザー・システムの永続環境変数は変更しません。

ライブ試験では、同じPowerShellプロセスで明示的に読み込んでから対象テストを実行します。

```powershell
& ./dev/scripts/import-EveryDB-development-environment.ps1 -Required
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --ignored airtable_live
```

`npm run check:all` とCIはこのファイルを読み込みません。通常テストは生成した架空データを使います。

## 管理上の注意

- 保管フォルダーは開発者本人と必要な管理主体だけが読める権限にします。
- アプリ内で利用者が登録する認証情報は従来どおりDPAPIで保護されます。本手順は開発・ライブ試験用です。
- Git除外は誤登録の予防です。既に登録した秘密値は失効・再発行し、Git履歴と公開成果物からも除去します。
- 端末移行では安全な経路で別途移送し、Gitや公開バックアップへ含めません。
