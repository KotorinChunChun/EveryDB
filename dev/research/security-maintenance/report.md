# 日次セキュリティ保守

## 2026-10-30

Vite6.4.4→6.4.5、libz-sys1.1.29→1.1.31の同系列修正を採用。合成deny5確認、Vitest158件、Rust214件/16ignore、正規check:allの全ゲートに成功。npm監査0、更新2版OSV0。稼働環境へ反映/配布しない。

[版別の計画・検証結果](../../history/v0.1.2/v0.1.2-imp.md)と[横断の当日報告](C:/develop/default/dev/research/security-maintenance/2026-10-30.md)を正本とする。一次公告は[Vite](https://github.com/vitejs/vite/security/advisories/GHSA-8g5g-mxqh-grgj)、[libz-sys](https://rustsec.org/advisories/RUSTSEC-2026-0418.html)。Unix/cc>=1.6というlibz条件と現行Windows/cc1.3を区分し、通常GUI/外部サービスは未確認とする。

## 2026-11-04 calamineの新着・承認K待ち

calamine0.37.0がRUSTSEC-2026-0436の対象で、ExcelSheets/Range等の自動形式読込みから到達可能。修正版0.38.0の最小2ファイル候補を隔離し、正規check:allはVitest158/Rust214成功16ignoreを含め合格。通常ソースには未採用。XLSB日付型/XLS文字列/ODS文字の読み取り値も変わるため、利用者の互換性判断を先に求める。詳細な固定差分・検証・復旧案は[版別報告](../../history/v0.1.2/v0.1.2-imp.md)。実データ/OOM攻撃/GUI/外部DB/配布は未実施。
