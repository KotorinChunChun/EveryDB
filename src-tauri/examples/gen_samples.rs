//! ダミーデータ生成ツール（開発・検証用）。
//! 実行: `cargo run --example gen_samples`
//! ../samples 以下に xlsx / IME 辞書(UTF-16LE, UTF-8) / フォルダツリーを生成する。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rust_xlsxwriter::{Table, Workbook};

fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("samples")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = samples_dir();
    fs::create_dir_all(&dir)?;
    gen_xlsx(&dir)?;
    gen_msime_dict(&dir)?;
    gen_google_dict(&dir)?;
    gen_tree(&dir)?;
    println!("生成完了: {}", dir.display());
    Ok(())
}

/// employees.xlsx: 「社員」シート(テーブル状) + 「拠点」シート + 定数セル。
fn gen_xlsx(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut wb = Workbook::new();

    let ws = wb.add_worksheet().set_name("社員")?;
    let header = ["社員番号", "氏名", "部署", "拠点", "メール"];
    for (c, h) in header.iter().enumerate() {
        ws.write_string(0, c as u16, *h)?;
    }
    let rows = [
        ["E001", "田中太郎", "営業部", "東京", "tanaka@example.co.jp"],
        ["E002", "鈴木花子", "開発部", "大阪", "suzuki@example.co.jp"],
        ["E003", "佐藤一郎", "総務部", "東京", "sato@example.co.jp"],
        ["E004", "高橋美咲", "営業部", "名古屋", "takahashi@example.co.jp"],
        ["E005", "渡辺健", "開発部", "東京", "watanabe@example.co.jp"],
        ["E006", "伊藤さくら", "人事部", "大阪", "ito@example.co.jp"],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            ws.write_string((r + 1) as u32, c as u16, *v)?;
        }
    }

    // 真の Excel テーブル（ListObject）。excel_table コネクタの検証用に「社員」シートとは
    // 別のシートへ配置する（同一シートに詰めると使用範囲がテーブル領域まで広がり、生セル表
    // 読み取り側の見出し行に重複列名が混ざってしまうため）。
    let ws_tbl = wb.add_worksheet().set_name("テーブルサンプル")?;
    let table_header = ["社員番号", "氏名", "部署"];
    for (c, h) in table_header.iter().enumerate() {
        ws_tbl.write_string(0, c as u16, *h)?;
    }
    let table_rows = [
        ["T001", "山本次郎", "経理部"],
        ["T002", "小林陽子", "情報システム部"],
        ["T003", "加藤誠", "経理部"],
    ];
    for (r, row) in table_rows.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            ws_tbl.write_string((r + 1) as u32, c as u16, *v)?;
        }
    }
    let table = Table::new().set_name("社員テーブル");
    ws_tbl.add_table(0, 0, table_rows.len() as u32, table_header.len() as u16 - 1, &table)?;

    let ws2 = wb.add_worksheet().set_name("拠点")?;
    for (r, v) in ["拠点名", "東京", "大阪", "名古屋", "福岡"].iter().enumerate() {
        ws2.write_string(r as u32, 0, *v)?;
    }

    // excel_cell 検証用の定数シート（B2 に会社名）。
    let ws3 = wb.add_worksheet().set_name("設定")?;
    ws3.write_string(1, 0, "会社名")?;
    ws3.write_string(1, 1, "サンプル株式会社")?;

    wb.save(dir.join("employees.xlsx"))?;
    Ok(())
}

/// Microsoft IME ユーザー辞書エクスポート形式: UTF-16LE(BOM付) / タブ区切り / 読み・語・品詞。
fn gen_msime_dict(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lines = [
        "!Microsoft IME Dictionary Tool", // 先頭の ! はコメント行として無視される想定
        "とうきょう\t東京\t地名",
        "おおさか\t大阪\t地名",
        "なごや\t名古屋\t地名",
        "えぶりでーびー\tEveryDB\t固有名詞",
        "たなかたろう\t田中太郎\t人名",
    ];
    let text = lines.join("\r\n");
    // UTF-16LE + BOM を手で組み立てる（encoding_rs は UTF-16 エンコードを持たない）。
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    let mut f = fs::File::create(dir.join("msime_dict.txt"))?;
    f.write_all(&bytes)?;
    Ok(())
}

/// Google 日本語入力ユーザー辞書エクスポート形式: UTF-8 / タブ区切り / 読み・単語・品詞・コメント。
fn gen_google_dict(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let text = "\
とうきょう\t東京\t地名\t首都
おおさか\t大阪\t地名\t
えぶりでーびー\tEveryDB\t名詞\t社内ツール
すずきはなこ\t鈴木花子\t人名\t開発部
";
    fs::write(dir.join("google_dict.txt"), text)?;
    Ok(())
}

/// folder コネクタ検証用のネストしたフォルダツリー。
fn gen_tree(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let root = dir.join("tree");
    let files = [
        ("docs/仕様書.md", "# 仕様書\n"),
        ("docs/議事録_2026-07.txt", "議事録\n"),
        ("images/logo.png", "PNG"),
        ("data/customers.csv", "id,name\n1,foo\n"),
        ("data/backup/old.csv", "id\n1\n"),
        ("README.txt", "sample tree\n"),
    ];
    for (rel, content) in files {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(p, content)?;
    }
    Ok(())
}
