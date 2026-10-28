//! The fixture of a promoted card: its record in the cached Oracle Cards file, pretty-printed with the keys in the
//! file's order, as `zcat ... | jq` prints it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

use crate::bulk::w2a_open_elements;
use crate::cache::Cache;
use crate::error::CliError;

/// A JSON value that keeps the order of its keys.
#[derive(Debug, Clone, PartialEq)]
enum Ordered {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    Text(String),
    List(Vec<Ordered>),
    Object(Vec<(String, Ordered)>),
}

struct OrderedVisitor;

impl<'de> Visitor<'de> for OrderedVisitor {
    type Value = Ordered;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<Ordered, E> {
        Ok(Ordered::Null)
    }
    fn visit_bool<E>(self, value: bool) -> Result<Ordered, E> {
        Ok(Ordered::Bool(value))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Ordered, E> {
        Ok(Ordered::Number(value.into()))
    }
    fn visit_u64<E>(self, value: u64) -> Result<Ordered, E> {
        Ok(Ordered::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Ordered, E> {
        serde_json::Number::from_f64(value)
            .map(Ordered::Number)
            .ok_or_else(|| E::custom("a number that is not finite"))
    }
    fn visit_str<E>(self, value: &str) -> Result<Ordered, E> {
        Ok(Ordered::Text(value.to_owned()))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Ordered, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Ordered::List(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(Ordered::Object(entries))
    }
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OrderedVisitor)
    }
}

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Ordered::Null => serializer.serialize_unit(),
            Ordered::Bool(value) => serializer.serialize_bool(*value),
            Ordered::Number(value) => value.serialize(serializer),
            Ordered::Text(value) => serializer.serialize_str(value),
            Ordered::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Ordered::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

/// The part of a record that says which card it is.
#[derive(Deserialize)]
struct RecordId {
    oracle_id: Option<String>,
}

/// One JSON record as a fixture file: pretty-printed with two spaces, the keys in their order, a final newline.
pub fn w2a_pretty(bytes: &[u8]) -> Result<String, serde_json::Error> {
    let value: Ordered = serde_json::from_slice(bytes)?;
    let mut text = serde_json::to_string_pretty(&value)?;
    text.push('\n');
    Ok(text)
}

/// The Oracle Cards file of the cache.
pub fn w2a_oracle_cards(cache: &Cache) -> Result<PathBuf, CliError> {
    let manifest = cache.manifest()?;
    manifest
        .scryfall
        .get("oracle_cards")
        .filter(|record| cache.holds(&record.file))
        .map(|record| cache.path(&record.file.file))
        .ok_or_else(|| {
            CliError::Config(
                "the cache holds no Oracle Cards file; `mtg-cli fetch scryfall` downloads it"
                    .into(),
            )
        })
}

/// The fixture texts of the cards with these oracle ids that the file holds, by oracle id.
pub fn w2a_records(
    path: &Path,
    wanted: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, CliError> {
    let mut elements = w2a_open_elements(path)?;
    let broken =
        |error: &dyn fmt::Display| CliError::Config(format!("{}: {error}", path.display()));
    let mut found = BTreeMap::new();
    while found.len() < wanted.len() {
        let Some(bytes) = elements.next_element().map_err(|e| broken(&e))? else {
            break;
        };
        let Ok(RecordId {
            oracle_id: Some(id),
        }) = serde_json::from_slice(&bytes)
        else {
            continue;
        };
        if wanted.contains(&id) {
            found.insert(id, w2a_pretty(&bytes).map_err(|e| broken(&e))?);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    //! Goal: a record printed as a fixture is byte for byte the corpus fixture `jq` printed (key order, numbers,
    //! nested faces, non-ASCII text), and only the wanted cards of a bulk file, gzip or plain, array or lines, are read.
    #![allow(clippy::unwrap_used)]

    use std::io::Write as _;

    use rstest::rstest;

    use super::*;

    fn corpus(name: &str) -> String {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mtg-cards/tests/fixtures/corpus");
        std::fs::read_to_string(dir.join(name)).unwrap()
    }

    fn compact(text: &str) -> String {
        serde_json::to_string(&serde_json::from_str::<Ordered>(text).unwrap()).unwrap()
    }

    #[rstest]
    #[case("eureka_moment.json")]
    #[case("lightning_bolt.json")]
    #[case("fire_ice.json")]
    #[case("delver_of_secrets_insectile_aberration.json")]
    #[case("legolas_greenleaf.json")]
    fn a_compact_record_prints_as_its_corpus_fixture(#[case] name: &str) {
        let fixture = corpus(name);
        assert_eq!(w2a_pretty(compact(&fixture).as_bytes()).unwrap(), fixture);
    }

    #[rstest]
    #[case::lines_plain(false, false)]
    #[case::lines_gzip(false, true)]
    #[case::array_plain(true, false)]
    #[case::array_gzip(true, true)]
    fn only_the_wanted_records_are_read(#[case] array: bool, #[case] gzip: bool) {
        let records = [
            r#"{"oracle_id":"a","name":"Alpha","cmc":4.0}"#,
            r#"{"name":"Reversible","card_faces":[{"oracle_id":"b"}]}"#,
            r#"{"oracle_id":"c","name":"Gamma","cmc":1}"#,
        ];
        let body = if array {
            format!("[\n{}\n]", records.join(",\n"))
        } else {
            records.join("\n")
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cards");
        let mut file = std::fs::File::create(&path).unwrap();
        if gzip {
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
            encoder.write_all(body.as_bytes()).unwrap();
            encoder.finish().unwrap();
        } else {
            file.write_all(body.as_bytes()).unwrap();
        }
        let wanted: BTreeSet<String> = ["a", "b", "z"].map(String::from).into();
        let found = w2a_records(&path, &wanted).unwrap();
        assert_eq!(found.keys().collect::<Vec<_>>(), ["a"]);
        assert_eq!(
            found["a"],
            "{\n  \"oracle_id\": \"a\",\n  \"name\": \"Alpha\",\n  \"cmc\": 4.0\n}\n"
        );
    }
}
