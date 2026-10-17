# EveryDB 配布手順

EveryDB の配布物生成と外部公開を区別するための正本です。PowerShell は `pwsh` を使用します。現在の v0.1.0 配布物はコード署名なしです。

## 前提条件

- Node.js と npm
- Rust stable 1.77 以上
- Visual Studio C++ ビルドツールと WebView2 Runtime
- MSI 生成に必要な WiX、NSIS 生成に必要なツール（Tauri CLI が案内する構成）
- 外部公開時のみ、認証済みの GitHub CLI `gh`

## 1. リリース候補の確認

1. `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json` の版番号が一致することを確認する。
2. リポジトリ直下で品質ゲートを実行する。

```powershell
npm ci
npm run check:all
```

3. [テスト方法](../docs/development/testing.md) のリリース前手動確認と依存関係監査を実施し、未実施項目は版別報告に理由を残す。

## 2. 配布物のローカル生成

次のコマンドは Release ビルド後に MSI と NSIS インストーラーを生成します。外部公開は行いません。

```powershell
npm run tauri build
```

主な生成先は次のとおりです。

- 実行ファイル: `src-tauri/target/release/everydb.exe`
- MSI: `src-tauri/target/release/bundle/msi/`
- NSIS: `src-tauri/target/release/bundle/nsis/`

配布候補だけを Git 対象外の `release/` へ収集し、ファイル名、サイズ、SHA-256、Authenticode 署名状態を版別実装報告へ記録します。秘密鍵付き証明書をリポジトリへ置いてはいけません。

## 3. 外部公開

公開先は [GitHub Releases](https://github.com/KotorinChunChun/EveryDB/releases) です。外部公開はユーザーが明示的に依頼した場合だけ行います。現時点では標準命名に準拠した `publish-release-EveryDB-*` スクリプトはありません。

公開時は、確定したタグとリリースノートを確認し、検証済みの MSI／NSIS だけをアップロードします。既存リリースの置換、タグ作成、push、署名はそれぞれ明示された範囲で行います。パッケージ生成またはアップロードの一方が失敗した場合、公開成功として記録しません。

## 4. 署名と未署名配布

v0.1.0 は未署名です。未署名の配布物では README の SmartScreen 注意を維持し、利用者が配布元と SHA-256 を確認できるようにします。将来署名を導入する場合は、証明書の保管、Publisher、タイムスタンプ、検証コマンドを本書へ追記してから実行します。

## 5. 資料の完了

リリースだけを理由に版別資料を完了扱いにしません。要求、実装報告、タスクの完了記録と未完了事項の引き継ぎを確認し、進行中・未来・完了済みの資料を最初から `dev/history/vX.Y.Z/` へ置き、完了後も移動しません。
