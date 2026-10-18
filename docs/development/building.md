# ビルド方法

## 前提条件

- Node.js（npmを含む）
- Rust stable（`rust-version = 1.77` 以上）
- Windows向けのC++ビルドツールとWebView2 Runtime

## セットアップ

```powershell
npm ci
```

## フロントエンドの確認

```powershell
npm run check:all
```

`check:all` はフロントエンド・公開文書・Rustのオフライン品質ゲートをまとめて実行します。

## Tauri開発起動

```powershell
pwsh -File .\dev\scripts\run-debug-EveryDB-all.ps1
```

KotorinToys本体から起動する管理連携版は、専用の出力先にビルドします。

```powershell
pwsh -File .\dev\scripts\build-debug-EveryDB-kotorintoys.ps1
```

ローカルサンプルを生成・登録して起動する場合は `run-debug-EveryDB-all-sample.ps1` を使用します。この入口は実際のユーザープロファイルの EveryDB キャッシュへ試験ソースを登録するため、通常起動と区別してください。

## Release構成の起動

Release 構成をビルドして起動する場合は、インストーラーを生成しない次の入口を使用します。

```powershell
pwsh -File .\dev\scripts\run-release-EveryDB-all.ps1
```

既存の Release 実行ファイルをビルドせず起動する場合は次を使用します。成果物が存在しない場合は失敗し、暗黙にビルドしません。

```powershell
pwsh -File .\dev\scripts\run-exe-EveryDB-all.ps1
```

## 配布物のビルド

```powershell
pwsh -File .\dev\scripts\build-release-EveryDB-kotorintoys.ps1
```

この入口はネイティブReleaseのみを生成します。共通Innoの生成はKotorinToys側の配布工程で行います。配布前は [EveryDB 配布手順](../../dev/RELEASE_RULE.md) を参照してください。

Rustだけを確認する場合は、リポジトリ直下から次を実行します。

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --bins
```
