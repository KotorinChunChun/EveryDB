//! 外部の公開データを取得するconnector群。

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;

use super::common::{make_record, CommonCfg};
use super::default_true;
use super::limits::{self, static_field_names};
use super::Connector;
use crate::connectors::json_scalar_to_string;
use crate::model::Record;
// ---- PokéAPI ----

#[derive(Deserialize)]
pub(crate) struct PokeApiConnector {
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for PokeApiConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let query = r#"
query {
  pokemon_v2_pokemon(order_by: {id: asc}) {
    id
    name
    pokemon_v2_pokemonspecy {
      pokemon_v2_pokemonspeciesnames(where: {language_id: {_eq: 11}}) {
        name
      }
    }
    pokemon_v2_pokemontypes {
      pokemon_v2_type {
        pokemon_v2_typenames(where: {language_id: {_eq: 11}}) {
          name
        }
      }
    }
    pokemon_v2_pokemonabilities {
      pokemon_v2_ability {
        pokemon_v2_abilitynames(where: {language_id: {_eq: 11}}) {
          name
        }
      }
    }
    pokemon_v2_pokemonstats {
      base_stat
    }
  }
}
        "#;
        let body = serde_json::json!({ "query": query });
        let res = limits::http_post_json_with_headers(
            "https://beta.pokeapi.co/graphql/v1beta",
            &[("Content-Type", "application/json")],
            body,
        )?;
        let json_res: Value = res.into_json()?;

        let list = json_res["data"]["pokemon_v2_pokemon"]
            .as_array()
            .ok_or_else(|| anyhow!("PokeAPI: no data array"))?;

        let mut out = Vec::new();
        for p in list {
            let id = p["id"].as_i64().unwrap_or(0);

            // 和名
            let ja_name = p["pokemon_v2_pokemonspecy"]["pokemon_v2_pokemonspeciesnames"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|obj| obj["name"].as_str())
                .unwrap_or_else(|| p["name"].as_str().unwrap_or(""));

            // タイプ
            let types_arr = p["pokemon_v2_pokemontypes"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| {
                            t["pokemon_v2_type"]["pokemon_v2_typenames"]
                                .as_array()
                                .and_then(|names| names.first())
                                .and_then(|obj| obj["name"].as_str())
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            // とくせい
            let abilities_arr = p["pokemon_v2_pokemonabilities"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| {
                            a["pokemon_v2_ability"]["pokemon_v2_abilitynames"]
                                .as_array()
                                .and_then(|names| names.first())
                                .and_then(|obj| obj["name"].as_str())
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            // 種族値
            let stats = p["pokemon_v2_pokemonstats"].as_array();
            let mut stats_str = String::new();
            if let Some(s) = stats {
                if s.len() == 6 {
                    let hp = s[0]["base_stat"].as_i64().unwrap_or(0);
                    let atk = s[1]["base_stat"].as_i64().unwrap_or(0);
                    let def = s[2]["base_stat"].as_i64().unwrap_or(0);
                    let spa = s[3]["base_stat"].as_i64().unwrap_or(0);
                    let spd = s[4]["base_stat"].as_i64().unwrap_or(0);
                    let spe = s[5]["base_stat"].as_i64().unwrap_or(0);
                    stats_str = format!(
                        "HP:{} 攻:{} 防:{} 特攻:{} 特防:{} 早:{} (合計:{})",
                        hp,
                        atk,
                        def,
                        spa,
                        spd,
                        spe,
                        hp + atk + def + spa + spd + spe
                    );
                }
            }

            let default_open_ref = format!("https://zukan.pokemon.co.jp/detail/{:04}", id);

            let pairs = vec![
                ("図鑑番号".to_string(), id.to_string()),
                ("和名".to_string(), ja_name.to_string()),
                ("タイプ".to_string(), types_arr),
                ("とくせい".to_string(), abilities_arr),
                ("種族値".to_string(), stats_str),
                (
                    "英語名".to_string(),
                    p["name"].as_str().unwrap_or("").to_string(),
                ),
            ];

            let mut rec = make_record(pairs, &default_open_ref, &self.common);
            // label_column の指定がない場合は和名をラベル（ツリー上の表示名）にする
            if self.common.label_column.is_none() {
                rec.label = Some(ja_name.to_string());
            }
            limits::push_record(&mut out, rec, "PokéAPI")?;
        }

        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        static_field_names(&["図鑑番号", "和名", "タイプ", "とくせい", "種族値", "英語名"])
    }
}

// ---- 政府公開データ（郵便番号 / 法人番号）共通ヘルパー ----
// どちらも「ネットから zip を自動ダウンロード」「手動でダウンロード済みの zip/csv を指定」の
// 2 経路を持つため、zip 展開と HTTP バイト取得の処理を共通化する。

fn response_bytes(resp: ureq::Response) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    resp.into_reader().read_to_end(&mut buf)?;
    Ok(buf)
}

/// zip バイト列から最初に見つかった `.csv` エントリを展開する（署名ファイル `.asc` 等は無視）。
pub(crate) fn extract_first_csv_from_zip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| anyhow!("zip の展開に失敗しました: {e}"))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| anyhow!("zip 展開エラー: {e}"))?;
        if file.name().to_ascii_lowercase().ends_with(".csv") {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    Err(anyhow!("zip 内に CSV ファイルが見つかりません"))
}

/// `source=file` のとき、指定パスが `.zip` なら展開して中の CSV を、`.csv` ならそのまま読む。
fn read_csv_bytes_from_path(path: &str) -> Result<Vec<u8>> {
    let bytes =
        std::fs::read(path).map_err(|e| anyhow!("ファイルの読み込みに失敗しました: {e}"))?;
    if path.to_ascii_lowercase().ends_with(".zip") {
        extract_first_csv_from_zip(&bytes)
    } else {
        Ok(bytes)
    }
}

/// UTF-8 CSV（ヘッダーなし想定・政府公開データの生ファイル）を行の配列として読む。
pub(crate) fn read_csv_rows(bytes: &[u8]) -> Result<Vec<csv::StringRecord>> {
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| anyhow!("CSV の文字コードが UTF-8 ではありません（Shift_JIS 版を指定していないか確認してください）"))?;
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());
    let mut out = Vec::new();
    for rec in rdr.records() {
        out.push(rec?);
    }
    Ok(out)
}

// ---- 郵便番号データベース（日本郵便）----
const KEN_ALL_URL: &str =
    "https://www.post.japanpost.jp/service/search/zipcode/download/utf/zip/utf_ken_all.zip";

#[derive(Deserialize)]
pub(crate) struct PostalCodeConnector {
    /// "auto"(既定・自動ダウンロード) | "file"(手動でダウンロード済みの zip/csv を指定)
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for PostalCodeConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let bytes = if self.source.as_deref() == Some("file") {
            let path = self
                .path
                .as_deref()
                .filter(|p| !p.is_empty())
                .ok_or_else(|| anyhow!("『ファイル指定』を選んだ場合はパスの入力が必要です"))?;
            read_csv_bytes_from_path(path)?
        } else {
            let resp = limits::http_get(KEN_ALL_URL).map_err(|e| {
                anyhow!(
                    "郵便番号データのダウンロードに失敗しました: {e}。\
                     手動でダウンロードした zip/csv を『ファイル指定』で指定してください"
                )
            })?;
            extract_first_csv_from_zip(&response_bytes(resp)?)?
        };
        let rows = read_csv_rows(&bytes)?;
        let mut out = Vec::new();
        for rec in rows {
            let get = |i: usize| rec.get(i).unwrap_or("").to_string();
            // KEN_ALL.CSV(UTF-8版) の列位置（0-based）。§0-2 で実データ確認済み。
            let postal = get(2);
            let pairs = vec![
                ("郵便番号".to_string(), postal),
                ("都道府県".to_string(), get(6)),
                ("市区町村".to_string(), get(7)),
                ("町域".to_string(), get(8)),
                ("都道府県カナ".to_string(), get(3)),
                ("市区町村カナ".to_string(), get(4)),
                ("町域カナ".to_string(), get(5)),
            ];
            out.push(make_record(pairs, "", &self.common));
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        static_field_names(&[
            "郵便番号",
            "都道府県",
            "市区町村",
            "町域",
            "都道府県カナ",
            "市区町村カナ",
            "町域カナ",
        ])
    }
}

// ---- 法人番号 全件データ（国税庁 法人番号公表サイト）----
const HOUJIN_INDEX_URL: &str = "https://www.houjin-bangou.nta.go.jp/download/zenken/index.html";
pub(crate) const HOUJIN_TOKEN_FIELD: &str =
    "jp.go.nta.houjin_bangou.framework.web.common.CNSFWTokenProcessor.request.token";

#[derive(Deserialize)]
pub(crate) struct CorporateNumberConnector {
    /// 都道府県名（サイト表示のとおり。例: "岐阜県"、全国分なら "全国"）。
    prefecture: String,
    /// "auto"(既定・自動ダウンロード) | "file"(手動でダウンロード済みの zip/csv を指定)
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for CorporateNumberConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let bytes = if self.source.as_deref() == Some("file") {
            let path = self
                .path
                .as_deref()
                .filter(|p| !p.is_empty())
                .ok_or_else(|| anyhow!("『ファイル指定』を選んだ場合はパスの入力が必要です"))?;
            read_csv_bytes_from_path(path)?
        } else {
            let zip_bytes = houjin_bangou_download(&self.prefecture).map_err(|e| {
                anyhow!(
                    "{e}。サイト構造の変更、またはネットワーク要因が考えられます。\
                     手動でダウンロードした zip を『ファイル指定』で指定してください"
                )
            })?;
            extract_first_csv_from_zip(&zip_bytes)?
        };
        let rows = read_csv_rows(&bytes)?;
        let mut out = Vec::new();
        for rec in rows {
            let get = |i: usize| rec.get(i).unwrap_or("").to_string();
            // 全件データ CSV の列位置（0-based）。実データの値から逆引きで確認した対応（§0-2）。
            let name = get(6);
            let pairs = vec![
                ("法人番号".to_string(), get(1)),
                ("商号又は名称".to_string(), name.clone()),
                ("法人種別".to_string(), get(8)),
                ("都道府県".to_string(), get(9)),
                ("市区町村".to_string(), get(10)),
                ("丁目番地等".to_string(), get(11)),
                ("郵便番号".to_string(), get(15)),
                ("更新年月日".to_string(), get(4)),
                ("法人番号指定年月日".to_string(), get(22)),
                ("フリガナ".to_string(), get(28)),
                ("英語表記(名称)".to_string(), get(24)),
                ("英語表記(都道府県)".to_string(), get(25)),
                ("英語表記(所在地)".to_string(), get(26)),
            ];
            let mut r = make_record(pairs, "", &self.common);
            if self.common.label_column.is_none() {
                r.label = Some(name);
            }
            out.push(r);
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        static_field_names(&[
            "法人番号",
            "商号又は名称",
            "法人種別",
            "都道府県",
            "市区町村",
            "丁目番地等",
            "郵便番号",
            "更新年月日",
            "法人番号指定年月日",
            "フリガナ",
            "英語表記(名称)",
            "英語表記(都道府県)",
            "英語表記(所在地)",
        ])
    }
}

/// 法人番号公表サイトの一覧ページから CSRF トークンと都道府県の ID を取得し、
/// 同一セッション（Cookie）で POST してダウンロードする（§0-2 で実測確認済みの手順）。
fn houjin_bangou_download(prefecture: &str) -> Result<Vec<u8>> {
    let agent = limits::http_agent();
    let resp = agent
        .get(HOUJIN_INDEX_URL)
        .call()
        .map_err(|e| anyhow!("法人番号公表サイトへの接続に失敗しました: {e}"))?;
    let body = resp
        .into_string()
        .map_err(|e| anyhow!("ページの読み込みに失敗しました: {e}"))?;
    let token = extract_between(
        &body,
        &format!("name=\"{HOUJIN_TOKEN_FIELD}\" value=\""),
        "\"",
    )
    .ok_or_else(|| anyhow!("ページ内に CSRF トークンが見つかりません"))?;
    let file_no = extract_file_no_for_prefecture(&body, prefecture)
        .ok_or_else(|| anyhow!("都道府県『{prefecture}』のダウンロードIDが見つかりません"))?;
    let resp2 = agent
        .post(HOUJIN_INDEX_URL)
        .send_form(&[
            (HOUJIN_TOKEN_FIELD, token.as_str()),
            ("selDlFileNo", file_no.as_str()),
            ("event", "download"),
        ])
        .map_err(|e| anyhow!("ダウンロードのリクエストに失敗しました: {e}"))?;
    response_bytes(resp2)
}

pub(crate) fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)? + start.len();
    let rest = &s[i..];
    let j = rest.find(end)?;
    Some(rest[..j].to_string())
}

/// 「csv-unicode」セクション内で、指定した都道府県名の `<dt>` 直後にある `doDownload(ID)` を探す。
/// 同じ都道府県でも形式（Shift-JIS/Unicode/XML）ごとに ID が異なるため、セクションで範囲を絞る。
pub(crate) fn extract_file_no_for_prefecture(html: &str, prefecture: &str) -> Option<String> {
    let sec_start = html.find("id=\"csv-unicode\"")?;
    let sec_end = html[sec_start..]
        .find("id=\"xml-unicode\"")
        .map(|i| sec_start + i)
        .unwrap_or(html.len());
    let section = &html[sec_start..sec_end];
    let dt_marker = format!("<dt class=\"mb05\">{prefecture}</dt>");
    let dt_pos = section.find(&dt_marker)?;
    let after = &section[dt_pos + dt_marker.len()..];
    let call_pos = after.find("doDownload(")?;
    let after_call = &after[call_pos + "doDownload(".len()..];
    let end_pos = after_call.find(')')?;
    Some(after_call[..end_pos].trim().to_string())
}

// ---- WEBサイトの情報をデータソース化（要件: HTML表 / CSS セレクタ / JSON API の3モード）----
// 実測（§0-2）: 例に挙がった zenn.dev の検索結果ページは JS 描画で HTML に結果が無く、
// 実データは JSON API から取得されている。「ページの HTML をそのまま取り込む」単一方式では
// この具体例そのものが満たせないため、3モードを用意する。

const JS_RENDERED_HINT: &str =
    "このページは JavaScript で描画されており HTML に結果が含まれていない可能性があります。\
     ブラウザの開発者ツールの Network タブで実際のデータ取得先 API を確認し、\
     json_api モードで指定してください。";

#[derive(Deserialize)]
pub(crate) struct WebPageConnector {
    url: String,
    #[serde(default = "default_web_page_mode")]
    mode: String, // "html_table" | "css_selector" | "json_api"
    #[serde(default)]
    table_index: Option<u32>,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default)]
    row_selector: Option<String>,
    /// css_selector: 1行に「表示名=セレクタ[@属性名]」/ json_api: 1行に「表示名=ドットパス」。
    #[serde(default)]
    columns: Option<String>,
    /// json_api: レスポンスから配列を取り出すドットパス。空欄ならレスポンス自体が配列。
    #[serde(default)]
    json_root: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}

fn default_web_page_mode() -> String {
    "html_table".to_string()
}

impl Connector for WebPageConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let resp = limits::http_get(&self.url);
        let body = match resp {
            Ok(r) => r
                .into_string()
                .map_err(|e| anyhow!("本文の読み込みに失敗しました: {e}"))?,
            Err(e) => return Err(anyhow!("URL 取得失敗: {e}")),
        };
        match self.mode.as_str() {
            "json_api" => self.fetch_json_api(&body),
            "css_selector" => self.fetch_css_selector(&body),
            _ => self.fetch_html_table(&body),
        }
    }

    fn field_names(&self) -> Result<Vec<String>> {
        match self.mode.as_str() {
            "css_selector" => {
                let spec = self.columns.as_deref().unwrap_or("").trim();
                if spec.is_empty() {
                    return Err(anyhow!(
                        "css_selector モードには columns の指定が必要です（例: タイトル=h2 a）"
                    ));
                }
                Ok(parse_css_columns(spec)
                    .into_iter()
                    .map(|(name, _, _)| name)
                    .collect())
            }
            "json_api" => {
                let spec = self.columns.as_deref().unwrap_or("").trim();
                let cols = parse_plain_columns(spec);
                if !cols.is_empty() {
                    return Ok(cols.into_iter().map(|(name, _)| name).collect());
                }
                Ok(Vec::new())
            }
            _ => Connector::field_names(self),
        }
    }
}

impl WebPageConnector {
    fn fetch_html_table(&self, body: &str) -> Result<Vec<Record>> {
        let doc = scraper::Html::parse_document(body);
        let table_sel = scraper::Selector::parse("table")
            .map_err(|e| anyhow!("内部エラー(tableセレクタ): {e:?}"))?;
        let tr_sel =
            scraper::Selector::parse("tr").map_err(|e| anyhow!("内部エラー(trセレクタ): {e:?}"))?;
        let cell_sel = scraper::Selector::parse("th,td")
            .map_err(|e| anyhow!("内部エラー(th,tdセレクタ): {e:?}"))?;
        let idx = self.table_index.unwrap_or(0) as usize;
        let table_count = doc.select(&table_sel).count();
        let table = doc.select(&table_sel).nth(idx).ok_or_else(|| {
            anyhow!("{idx} 番目の <table> が見つかりません（ページ内に table は {table_count} 個）。{JS_RENDERED_HINT}")
        })?;

        let mut headers: Vec<String> = Vec::new();
        let mut out = Vec::new();
        let mut first_row = true;
        for tr in table.select(&tr_sel) {
            let cells: Vec<String> = tr
                .select(&cell_sel)
                .map(|c| c.text().collect::<Vec<_>>().join("").trim().to_string())
                .collect();
            if cells.is_empty() {
                continue;
            }
            if first_row && self.has_header {
                headers = cells;
                first_row = false;
                continue;
            }
            first_row = false;
            let pairs: Vec<(String, String)> = cells
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    (
                        headers
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("列{}", i + 1)),
                        v,
                    )
                })
                .collect();
            out.push(make_record(pairs, &self.url, &self.common));
        }
        if out.is_empty() {
            return Err(anyhow!(
                "テーブルからデータ行を取得できませんでした。{JS_RENDERED_HINT}"
            ));
        }
        Ok(out)
    }

    fn fetch_css_selector(&self, body: &str) -> Result<Vec<Record>> {
        let doc = scraper::Html::parse_document(body);
        let row_selector_str = self
            .row_selector
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| anyhow!("css_selector モードには row_selector の指定が必要です"))?;
        let row_sel = scraper::Selector::parse(row_selector_str)
            .map_err(|e| anyhow!("row_selector の解析に失敗しました: {e:?}"))?;
        let columns_spec = self
            .columns
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                anyhow!("css_selector モードには columns の指定が必要です（例: タイトル=h2 a）")
            })?;
        let parsed = parse_css_columns(columns_spec);
        if parsed.is_empty() {
            return Err(anyhow!("columns の書式が正しくありません（1行に『表示名=セレクタ』、属性を取るなら『表示名=セレクタ@属性名』）"));
        }
        let compiled: Vec<(String, scraper::Selector, Option<String>)> = parsed
            .into_iter()
            .map(|(name, sel, attr)| {
                scraper::Selector::parse(&sel)
                    .map(|s| (name.clone(), s, attr))
                    .map_err(|e| anyhow!("『{name}』のセレクタが不正です: {e:?}"))
            })
            .collect::<Result<_>>()?;

        let mut out = Vec::new();
        for row in doc.select(&row_sel) {
            let pairs: Vec<(String, String)> = compiled
                .iter()
                .map(|(name, sel, attr)| {
                    let value = row
                        .select(sel)
                        .next()
                        .map(|el| match attr {
                            Some(a) => el.value().attr(a).unwrap_or("").to_string(),
                            None => el.text().collect::<Vec<_>>().join("").trim().to_string(),
                        })
                        .unwrap_or_default();
                    (name.clone(), value)
                })
                .collect();
            out.push(make_record(pairs, &self.url, &self.common));
        }
        if out.is_empty() {
            return Err(anyhow!(
                "row_selector に一致する要素が見つかりませんでした。{JS_RENDERED_HINT}"
            ));
        }
        Ok(out)
    }

    fn fetch_json_api(&self, body: &str) -> Result<Vec<Record>> {
        let root: Value =
            serde_json::from_str(body).map_err(|e| anyhow!("JSON の解析に失敗しました: {e}"))?;
        let root_path = self.json_root.as_deref().unwrap_or("").trim();
        let target = json_path_get(&root, root_path)
            .ok_or_else(|| anyhow!("json_root『{root_path}』に一致するデータが見つかりません"))?;
        let array = target
            .as_array()
            .ok_or_else(|| anyhow!("json_root で指定した先が配列ではありません"))?;
        let columns_spec = self.columns.as_deref().unwrap_or("").trim();
        let cols = parse_plain_columns(columns_spec);

        let mut out = Vec::new();
        for item in array {
            let pairs: Vec<(String, String)> = if !cols.is_empty() {
                cols.iter()
                    .map(|(name, path)| {
                        let v = json_path_get(item, path);
                        (
                            name.clone(),
                            v.map(json_scalar_to_string).unwrap_or_default(),
                        )
                    })
                    .collect()
            } else if let Some(obj) = item.as_object() {
                obj.iter()
                    .map(|(k, v)| (k.clone(), json_scalar_to_string(v)))
                    .collect()
            } else {
                vec![("値".to_string(), json_scalar_to_string(item))]
            };
            out.push(make_record(pairs, &self.url, &self.common));
        }
        if out.is_empty() {
            return Err(anyhow!(
                "json_root で指定した配列は取得できましたが要素が 0 件でした"
            ));
        }
        Ok(out)
    }
}

/// css_selector 用の列定義パーサー。1 行 = 「表示名=セレクタ[@属性名]」。
fn parse_css_columns(spec: &str) -> Vec<(String, String, Option<String>)> {
    spec.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (name, rest) = line.split_once('=')?;
            let rest = rest.trim();
            if let Some((sel, attr)) = rest.rsplit_once('@') {
                Some((
                    name.trim().to_string(),
                    sel.trim().to_string(),
                    Some(attr.trim().to_string()),
                ))
            } else {
                Some((name.trim().to_string(), rest.to_string(), None))
            }
        })
        .collect()
}

/// json_api 用の列定義パーサー。1 行 = 「表示名=ドットパス」（属性の概念は無い）。
fn parse_plain_columns(spec: &str) -> Vec<(String, String)> {
    spec.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (name, path) = line.split_once('=')?;
            Some((name.trim().to_string(), path.trim().to_string()))
        })
        .collect()
}

/// ドット区切りの JSON パスをたどる（配列は数値インデックスで指定）。空文字はルート自身を返す。
fn json_path_get<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }
    let mut cur = value;
    for part in path.split('.') {
        if let Ok(idx) = part.parse::<usize>() {
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(part)?;
        }
    }
    Some(cur)
}

#[cfg(test)]
mod gov_data_tests {
    use super::*;
    use crate::connectors::build_connector;

    fn fetch(type_: &str, cfg: serde_json::Value) -> Vec<Record> {
        build_connector(type_, &cfg.to_string())
            .unwrap()
            .fetch()
            .unwrap()
    }

    /// 実サイトの構造を模した最小限の HTML（3 形式セクション、同一都道府県でも ID が異なる）。
    const MOCK_HOUJIN_HTML: &str = concat!(
        r#"<form id="appForm" action="/download/zenken/index.html" method="post">"#,
        r#"<input type="hidden" name="jp.go.nta.houjin_bangou.framework.web.common.CNSFWTokenProcessor.request.token" value="mock-token-123"/>"#,
        r#"</form>"#,
        r#"<h2 id="csv-sjis">CSV形式・Shift_JIS</h2>"#,
        r#"<dt class="mb05">岐阜県</dt><a onclick="return doDownload(11111);">zip</a>"#,
        r#"<h2 id="csv-unicode">CSV形式・Unicode</h2>"#,
        r#"<dt class="mb05">全国</dt><a onclick="return doDownload(20000);">zip</a>"#,
        r#"<dt class="mb05">岐阜県</dt><a onclick="return doDownload(27505);">zip</a>"#,
        r#"<h2 id="xml-unicode">XML形式・Unicode</h2>"#,
        r#"<dt class="mb05">岐阜県</dt><a onclick="return doDownload(33333);">zip</a>"#,
    );

    #[test]
    fn extracts_token_from_mock_page() {
        let token = extract_between(
            MOCK_HOUJIN_HTML,
            &format!("name=\"{HOUJIN_TOKEN_FIELD}\" value=\""),
            "\"",
        );
        assert_eq!(token.as_deref(), Some("mock-token-123"));
    }

    #[test]
    fn picks_file_no_from_csv_unicode_section_only() {
        // csv-sjis(11111) / xml-unicode(33333) ではなく csv-unicode(27505) が選ばれること。
        assert_eq!(
            extract_file_no_for_prefecture(MOCK_HOUJIN_HTML, "岐阜県").as_deref(),
            Some("27505")
        );
        assert_eq!(
            extract_file_no_for_prefecture(MOCK_HOUJIN_HTML, "全国").as_deref(),
            Some("20000")
        );
        assert_eq!(
            extract_file_no_for_prefecture(MOCK_HOUJIN_HTML, "存在しない県"),
            None
        );
    }

    fn build_test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut buf);
            let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, data) in entries {
                zw.start_file(*name, opts).unwrap();
                std::io::Write::write_all(&mut zw, data).unwrap();
            }
            zw.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn extract_first_csv_from_zip_ignores_asc_signature_file() {
        let zip_bytes = build_test_zip(&[
            ("21_gifu_all_20260630.csv.asc", b"signature-bytes-not-csv"),
            (
                "21_gifu_all_20260630.csv",
                "1,1000012160079,\"多治見検察審査会\"".as_bytes(),
            ),
        ]);
        let csv_bytes = extract_first_csv_from_zip(&zip_bytes).unwrap();
        assert_eq!(
            String::from_utf8(csv_bytes).unwrap(),
            "1,1000012160079,\"多治見検察審査会\""
        );
    }

    #[test]
    fn read_csv_rows_parses_quoted_japanese_fields() {
        let rows =
            read_csv_rows("1,1000012160079,\"多治見検察審査会\",\"岐阜県\"\n".as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get(2), Some("多治見検察審査会"));
        assert_eq!(rows[0].get(3), Some("岐阜県"));
    }

    // 実ネットワークへ接続する実機検証。実行: cargo test --lib -- --ignored gov_data
    #[test]
    #[ignore = "requires network access; downloads real government data"]
    fn postal_code_live_downloads_and_parses() {
        let recs = fetch("postal_code", serde_json::json!({}));
        println!("postal_code rows = {}", recs.len());
        assert!(!recs.is_empty());
        assert!(recs
            .iter()
            .any(|r| r.fields.get("郵便番号").and_then(|v| v.as_str()) == Some("5070023")));
    }

    #[test]
    #[ignore = "requires network access; downloads real government data (岐阜県)"]
    fn corporate_number_live_downloads_gifu() {
        let recs = fetch(
            "corporate_number",
            serde_json::json!({ "prefecture": "岐阜県" }),
        );
        println!("corporate_number (岐阜県) rows = {}", recs.len());
        assert!(!recs.is_empty());
        assert!(recs
            .iter()
            .any(|r| r.fields.get("法人番号").and_then(|v| v.as_str()) == Some("1000012160079")));
    }
}

#[cfg(test)]
mod web_page_tests {
    use super::*;
    use crate::connectors::build_connector;

    /// ネットワークへは繋がず、パース処理（fetch_html_table 等）を直接呼ぶための素の接続子。
    fn conn(mode: &str) -> WebPageConnector {
        WebPageConnector {
            url: "https://example.com/mock".to_string(),
            mode: mode.to_string(),
            table_index: None,
            has_header: true,
            row_selector: None,
            columns: None,
            json_root: None,
            common: CommonCfg::default(),
        }
    }

    const MOCK_TABLE_HTML: &str = r#"
        <html><body>
          <p>本文中の table ではない部分</p>
          <table>
            <tr><th>名前</th><th>URL</th></tr>
            <tr><td>記事A</td><td>https://example.com/a</td></tr>
            <tr><td>記事B</td><td>https://example.com/b</td></tr>
          </table>
        </body></html>
    "#;

    #[test]
    fn html_table_mode_treats_markup_as_text_not_executable() {
        let html = r#"<table><tr><th>本文</th></tr><tr><td><script>alert(1)</script>安全</td></tr></table>"#;
        let recs = conn("html_table").fetch_html_table(html).unwrap();
        assert_eq!(recs.len(), 1);
        let cell = recs[0]
            .fields
            .get("本文")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(cell.contains("安全"));
        assert!(
            !cell.contains('<'),
            "HTML tags must not survive ingest as markup: {cell}"
        );
    }

    #[test]
    fn html_table_mode_reads_header_and_rows() {
        let recs = conn("html_table")
            .fetch_html_table(MOCK_TABLE_HTML)
            .unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(
            recs[0].fields.get("名前").and_then(|v| v.as_str()),
            Some("記事A")
        );
        assert_eq!(
            recs[0].fields.get("URL").and_then(|v| v.as_str()),
            Some("https://example.com/a")
        );
    }

    #[test]
    fn html_table_mode_errors_with_js_rendered_hint_when_no_table() {
        let err = conn("html_table")
            .fetch_html_table("<html><body>結果はここにはありません</body></html>")
            .map(|_| ())
            .unwrap_err();
        assert!(err.to_string().contains("JavaScript"));
    }

    const MOCK_LIST_HTML: &str = r#"
        <html><body>
          <div class="not-a-post">無視されるべき要素</div>
          <article class="post"><h2><a href="/p/1">記事1</a></h2><span class="date">2026-01-01</span></article>
          <article class="post"><h2><a href="/p/2">記事2</a></h2><span class="date">2026-01-02</span></article>
        </body></html>
    "#;

    #[test]
    fn css_selector_mode_extracts_text_and_attribute() {
        let mut c = conn("css_selector");
        c.row_selector = Some("article.post".to_string());
        c.columns = Some("タイトル=h2 a\nURL=h2 a@href\n日付=span.date".to_string());
        let recs = c.fetch_css_selector(MOCK_LIST_HTML).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(
            recs[0].fields.get("タイトル").and_then(|v| v.as_str()),
            Some("記事1")
        );
        assert_eq!(
            recs[0].fields.get("URL").and_then(|v| v.as_str()),
            Some("/p/1")
        );
        assert_eq!(
            recs[0].fields.get("日付").and_then(|v| v.as_str()),
            Some("2026-01-01")
        );
    }

    #[test]
    fn css_selector_mode_errors_when_row_selector_missing() {
        let mut c = conn("css_selector");
        c.columns = Some("a=b".to_string());
        assert!(c.fetch_css_selector(MOCK_LIST_HTML).is_err());
    }

    const MOCK_JSON_BODY: &str = r#"{
        "articles": [
            { "title": "VBAの話", "liked_count": 3, "user": { "name": "たろう" } },
            { "title": "別の記事", "liked_count": 1, "user": { "name": "はなこ" } }
        ]
    }"#;

    #[test]
    fn json_api_mode_flattens_top_level_keys_when_columns_empty() {
        // zenn.dev の実測構造を模した最小 JSON（articles 配列、各要素はフラットなオブジェクト）。
        let mut c = conn("json_api");
        c.json_root = Some("articles".to_string());
        let recs = c.fetch_json_api(MOCK_JSON_BODY).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(
            recs[0].fields.get("title").and_then(|v| v.as_str()),
            Some("VBAの話")
        );
        assert_eq!(
            recs[0].fields.get("liked_count").and_then(|v| v.as_str()),
            Some("3")
        );
    }

    #[test]
    fn json_api_mode_uses_explicit_dot_path_columns() {
        let mut c = conn("json_api");
        c.json_root = Some("articles".to_string());
        c.columns = Some("タイトル=title\nいいね=liked_count\n投稿者=user.name".to_string());
        let recs = c.fetch_json_api(MOCK_JSON_BODY).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(
            recs[0].fields.get("タイトル").and_then(|v| v.as_str()),
            Some("VBAの話")
        );
        assert_eq!(
            recs[0].fields.get("投稿者").and_then(|v| v.as_str()),
            Some("たろう")
        );
    }

    #[test]
    fn json_api_mode_root_array_when_json_root_empty() {
        let recs = conn("json_api")
            .fetch_json_api(r#"[{"a":1},{"a":2}]"#)
            .unwrap();
        assert_eq!(recs.len(), 2);
    }

    #[test]
    fn parse_css_columns_splits_name_selector_and_optional_attribute() {
        let cols = parse_css_columns("タイトル=h2 a\nURL=a@href\n\n  空行は無視  =x");
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0], ("タイトル".to_string(), "h2 a".to_string(), None));
        assert_eq!(
            cols[1],
            ("URL".to_string(), "a".to_string(), Some("href".to_string()))
        );
    }

    #[test]
    fn json_path_get_navigates_dots_and_array_indices() {
        let v = serde_json::json!({ "a": { "b": [10, 20, { "c": "x" }] } });
        assert_eq!(json_path_get(&v, "a.b.0"), Some(&serde_json::json!(10)));
        assert_eq!(json_path_get(&v, "a.b.2.c"), Some(&serde_json::json!("x")));
        assert_eq!(json_path_get(&v, "a.missing"), None);
        assert_eq!(json_path_get(&v, ""), Some(&v));
    }

    // 実ネットワークへ接続する実機検証。実行: cargo test --lib -- --ignored web_page
    #[test]
    #[ignore = "requires network access; hits the real zenn.dev search API"]
    fn zenn_search_api_live() {
        let recs = build_connector(
            "web_page",
            &serde_json::json!({
                "url": "https://zenn.dev/api/search?q=vba&source=articles",
                "mode": "json_api",
                "json_root": "articles",
                "columns": "タイトル=title\nいいね=liked_count",
            })
            .to_string(),
        )
        .unwrap()
        .fetch()
        .unwrap();
        println!("zenn articles = {}", recs.len());
        assert!(!recs.is_empty());
    }
}
