//! 外部の公開データを取得するconnector群。

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

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
