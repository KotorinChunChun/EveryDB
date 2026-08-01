//! 外部の公開データを取得するconnector群。

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;

use super::common::{make_record, CommonCfg};
use super::Connector;
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
        let res = ureq::post("https://beta.pokeapi.co/graphql/v1beta")
            .set("Content-Type", "application/json")
            .send_json(body)?;
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
            out.push(rec);
        }

        Ok(out)
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
            let resp = ureq::get(KEN_ALL_URL).call().map_err(|e| {
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
}

/// 法人番号公表サイトの一覧ページから CSRF トークンと都道府県の ID を取得し、
/// 同一セッション（Cookie）で POST してダウンロードする（§0-2 で実測確認済みの手順）。
fn houjin_bangou_download(prefecture: &str) -> Result<Vec<u8>> {
    let agent = ureq::AgentBuilder::new().build();
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
