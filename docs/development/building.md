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
npm run tauri dev
```

## 配布物のビルド

```powershell
npm run tauri build
```

Rustだけを確認する場合は、リポジトリ直下から次を実行します。

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo build --manifest-path src-tauri/Cargo.toml --bins
```
