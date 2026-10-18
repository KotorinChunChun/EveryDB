# EveryDB 配布手順

0.1.1からWindows 11 x64向けに他の対応ツールと同じInno共通形式で配布する。コード署名は未実施。旧0.1.0のMSI／NSISとassetは変更しない。

## 候補の確認とビルド

Node.js/npm、Rust stable、Visual Studio C++ビルドツール、WebView2 Runtimeを用意する。PowerShellはpwshを使う。

1. package.json、package-lock.json、Cargo.toml、Cargo.lock、tauri.conf.json、tool.jsonの製品版を一致させる。
2. `npm ci`、`npm run check:all`、`pwsh -File dev/scripts/test-script-naming.ps1`を実行する。
3. [テスト方法](../docs/development/testing.md)に従って依存監査と手動受入を確認し、未実施範囲を版別impへ残す。
4. `pwsh -File dev/scripts/build-release-EveryDB-kotorintoys.ps1`でバンドルなしのReleaseを生成する。開発用秘密ファイルは読み込まない。

## 共通インストーラー

親KotorinToysが固定gitlinkから展開したソースに対し、`dev/scripts/publish-packages.ps1 -Products EveryDB`、`dev/scripts/build-installers.ps1`を実行する。前者がEveryDB.exe、サンプル、tool.json、文書、利用条件、[第三者表示](../licenses/README.md)、共通maintenanceとその.NET表示を収集し、後者が内容を照合する。任意のローカル作業中バイナリを差し込まない。

配布形式は `EveryDB-<版>-setup.exe`、`EveryDB-packages.json`、`EveryDB-SHA256SUMS.txt`。本体0.1.5以降が固定リポジトリ・製品識別・版・サイズ・SHA-256を照合する。旧MSI／NSISの登録を自動削除しない。利用者データ `%APPDATA%\com.everydb.app` は導入先へ同梱せず、アンインストールでも保持する。

## 公開

ユーザーの明示指示があるときだけ、親のpublish-releases工程または同等の照合を通して[EveryDB Releases](https://github.com/KotorinChunChun/EveryDB/releases)へ公開する。候補の固定コミット・タグ、未署名状態、ファイル名・サイズ・SHA-256、監査の残存事項・未検証範囲を版別報告と日本語ノートへ記録する。未受入の範囲があればPre-releaseとする。LICENSE.mdとTHIRD_PARTY_NOTICES.txtも添付する。

公開後は匿名HTTPで全assetのサイズ・SHA-256を再照合する。既存assetやタグを上書きせず、履歴整理前のEveryDB-private-archiveはPrivate・archivedを維持する。秘密値と署名用秘密鍵はC:\AI\env\EveryDBへ置き、ソースやログへ出力しない。