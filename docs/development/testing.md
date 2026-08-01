# テスト方法

## 通常の自動検証

```powershell
npm run check
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --bins
```

## ライブテスト

Rustの一部テストは外部サービス、ローカルブラウザプロファイル、ODBCドライバ、認証情報を必要とするため `ignored` です。秘密情報はリポジトリや設定ファイルへ保存せず、実行時の環境変数だけで渡してください。

## 手動確認

Tauri実機確認、Windows Hello、インストーラーでの更新・アンインストールは対話操作を伴います。リリース候補の最終段階で実施し、確認できなかった項目は未検証として報告書へ残します。
