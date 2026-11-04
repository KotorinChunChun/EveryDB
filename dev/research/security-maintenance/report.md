# 日次セキュリティ保守

## 2026-10-30

Vite6.4.4→6.4.5、libz-sys1.1.29→1.1.31の同系列修正を採用。合成deny5確認、Vitest158件、Rust214件/16ignore、正規check:allの全ゲートに成功。npm監査0、更新2版OSV0。稼働環境へ反映/配布しない。

[版別の計画・検証結果](../../history/v0.1.2/v0.1.2-imp.md)と[横断の当日報告](C:/develop/default/dev/research/security-maintenance/2026-10-30.md)を正本とする。一次公告は[Vite](https://github.com/vitejs/vite/security/advisories/GHSA-8g5g-mxqh-grgj)、[libz-sys](https://rustsec.org/advisories/RUSTSEC-2026-0418.html)。Unix/cc>=1.6というlibz条件と現行Windows/cc1.3を区分し、通常GUI/外部サービスは未確認とする。

## 2026-11-04 calamineの新着・承認K待ち

calamine0.37.0がRUSTSEC-2026-0436の対象で、ExcelSheets/Range等の自動形式読込みから到達可能。修正版0.38.0の最小2ファイル候補を隔離し、正規check:allはVitest158/Rust214成功16ignoreを含め合格。通常ソースには未採用。XLSB日付型/XLS文字列/ODS文字の読み取り値も変わるため、利用者の互換性判断を先に求める。詳細な固定差分・検証・復旧案は[版別報告](../../history/v0.1.2/v0.1.2-imp.md)。実データ/OOM攻撃/GUI/外部DB/配布は未実施。

## 2026-11-05 承認Kによる採用完了

ユーザーの明示承認に基づきcalamine0.37.0→0.38.0のCargo.toml/Cargo.lockを承認済み候補と完全一致する形で採用。採用後の同一ソースから隔離した正規check:allが成功し、Vitest158件・Rust214件成功16ignore、型/Clippy/フロント・Rust build等に合格。npm監査0、calamineのOSV一致0。Rust全体の他OS/保守終了等の既存候補12群13IDは継続し、全依存が安全になったとはしない。

通常node_modules/利用者DB/稼働アプリは未変更。配布・導入・再起動なし。承認済みの読取り値変化とGUI/実Excel/外部DB/Helloの未確認は維持する。[版別報告と証跡](../../history/v0.1.2/v0.1.2-imp.md) / [承認後の横断報告](C:/develop/default/dev/research/security-maintenance/2026-11-05-update.md)。
