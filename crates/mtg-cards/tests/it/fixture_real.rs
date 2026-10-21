//! Goal: every record of `corpus_scryfall.json` equals the Oracle Cards record with the same oracle id, so no id, printing or text is invented.
//!
//! The comparison runs on an `mtg-cli` cache or on `MTG_REAL_DATA_DIR`: `cargo test -p mtg-cards fixture_real:: -- --ignored`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use rstest::rstest;
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../fixtures/corpus_scryfall.json");

/// Fields of a record that must equal the real record, including the ones the fixture leaves out.
const FIELDS: [&str; 20] = [
    "id",
    "oracle_id",
    "lang",
    "layout",
    "set",
    "collector_number",
    "rarity",
    "released_at",
    "digital",
    "mana_cost",
    "cmc",
    "type_line",
    "oracle_text",
    "power",
    "toughness",
    "loyalty",
    "colors",
    "color_identity",
    "keywords",
    "produced_mana",
];

/// Fields of a face that must equal the real face, including the ones the fixture leaves out.
const FACE_FIELDS: [&str; 9] = [
    "name",
    "mana_cost",
    "type_line",
    "oracle_text",
    "power",
    "toughness",
    "loyalty",
    "defense",
    "colors",
];

/// The fields in which `fixture` differs from `real`, as `field: fixture ..., real ...`; a name is checked too.
fn differences(fixture: &Value, real: &Value) -> Vec<String> {
    let mut found: Vec<String> = ["name"]
        .into_iter()
        .chain(FIELDS)
        .filter_map(|field| unequal(field, &fixture[field], &real[field]))
        .collect();
    let (ours, theirs) = (face_list(fixture), face_list(real));
    if ours.len() != theirs.len() {
        found.push(format!(
            "card_faces: fixture has {} faces, real {}",
            ours.len(),
            theirs.len()
        ));
    }
    for (index, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
        found.extend(FACE_FIELDS.into_iter().filter_map(|field| {
            unequal(
                &format!("card_faces[{index}].{field}"),
                &ours[field],
                &theirs[field],
            )
        }));
    }
    found
}

fn unequal(path: &str, ours: &Value, theirs: &Value) -> Option<String> {
    (ours != theirs).then(|| format!("{path}: fixture {ours}, real {theirs}"))
}

fn face_list(record: &Value) -> Vec<Value> {
    record["card_faces"].as_array().cloned().unwrap_or_default()
}

/// The newest `oracle-cards-*.jsonl.gz` of `MTG_REAL_DATA_DIR`, else of the `mtg-cli` cache.
fn oracle_cards_file() -> PathBuf {
    let root = std::env::var_os("MTG_REAL_DATA_DIR")
        .or_else(|| std::env::var_os("MTG_CACHE_DIR"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_CACHE_HOME").map(|dir| PathBuf::from(dir).join("mtg-cli"))
        })
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache/mtg-cli")))
        .expect("set MTG_REAL_DATA_DIR, or fetch the cache with `mtg-cli fetch scryfall`");
    let mut files: Vec<PathBuf> = [root.join("scryfall"), root.clone()]
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .map(|entry| entry.unwrap().path())
        .filter(|path| is_oracle_cards(path))
        .collect();
    files.sort_by_key(|path| path.file_name().map(ToOwned::to_owned));
    files.pop().unwrap_or_else(|| {
        panic!(
            "no oracle-cards-*.jsonl.gz under {}; run `mtg-cli fetch scryfall`",
            root.display()
        )
    })
}

fn is_oracle_cards(path: &Path) -> bool {
    let name = path.file_name().unwrap().to_string_lossy();
    name.starts_with("oracle-cards-") && name.ends_with(".jsonl.gz")
}

/// The records of `file` whose oracle id is in `wanted`, by oracle id.
fn real_records(file: &Path, wanted: &[&str]) -> BTreeMap<String, Value> {
    let reader = BufReader::new(flate2::read::GzDecoder::new(BufReader::new(
        std::fs::File::open(file).unwrap(),
    )));
    reader
        .lines()
        .map(|line| line.unwrap())
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(&line).unwrap())
        .filter(|record| {
            record["oracle_id"]
                .as_str()
                .is_some_and(|id| wanted.contains(&id))
        })
        .map(|record| (record["oracle_id"].as_str().unwrap().to_owned(), record))
        .collect()
}

/// Ignored by default (needs the `mtg-cli` cache or `MTG_REAL_DATA_DIR`).
#[test]
#[ignore = "needs the mtg-cli cache or MTG_REAL_DATA_DIR"]
fn every_fixture_record_equals_the_cached_oracle_card() {
    let fixture: Vec<Value> = serde_json::from_str(FIXTURE).unwrap();
    let wanted: Vec<&str> = fixture
        .iter()
        .map(|record| record["oracle_id"].as_str().unwrap())
        .collect();
    let file = oracle_cards_file();
    let real = real_records(&file, &wanted);
    let mut problems = Vec::new();
    for record in &fixture {
        let name = record["name"].as_str().unwrap();
        match real.get(record["oracle_id"].as_str().unwrap()) {
            None => problems.push(format!(
                "{name}: oracle id {} is not in the cache",
                record["oracle_id"]
            )),
            Some(real) => problems.extend(
                differences(record, real)
                    .into_iter()
                    .map(|line| format!("{name}: {line}")),
            ),
        }
    }
    assert!(
        problems.is_empty(),
        "{} differ from {}:\n{}",
        problems.len(),
        file.display(),
        problems.join("\n")
    );
}

fn card() -> Value {
    json!({
        "id": "a", "oracle_id": "o", "name": "Fire // Ice", "layout": "split",
        "mana_cost": "{1}{R} // {1}{U}", "cmc": 4.0, "colors": ["R", "U"],
        "card_faces": [
            {"name": "Fire", "mana_cost": "{1}{R}", "oracle_text": "Fire."},
            {"name": "Ice", "mana_cost": "{1}{U}", "oracle_text": "Ice."}
        ]
    })
}

#[rstest]
#[case::the_same_record(json!({}), &[])]
#[case::an_other_oracle_id(json!({"oracle_id": "x"}), &["oracle_id"])]
#[case::a_set_the_real_card_lacks(json!({"set": "tmp"}), &["set"])]
#[case::colors_in_another_order(json!({"colors": ["U", "R"]}), &["colors"])]
#[case::a_cost_the_fixture_leaves_out(json!({"mana_cost": null}), &["mana_cost"])]
#[case::a_face_text(
    json!({"card_faces": [
        {"name": "Fire", "mana_cost": "{1}{R}", "oracle_text": "Fire!"},
        {"name": "Ice", "mana_cost": "{1}{U}", "oracle_text": "Ice."}
    ]}),
    &["card_faces[0].oracle_text"]
)]
#[case::a_missing_face(
    json!({"card_faces": [{"name": "Fire", "mana_cost": "{1}{R}", "oracle_text": "Fire."}]}),
    &["card_faces: fixture has 1 faces"]
)]
fn a_difference_names_the_field(#[case] change: Value, #[case] named: &[&str]) {
    let real = card();
    let mut fixture = card();
    for (key, value) in change.as_object().unwrap() {
        if value.is_null() {
            fixture.as_object_mut().unwrap().remove(key);
        } else {
            fixture[key] = value.clone();
        }
    }
    let found = differences(&fixture, &real);
    assert_eq!(found.len(), named.len(), "{found:?}");
    for (line, name) in found.iter().zip(named) {
        assert!(line.starts_with(name), "{line} does not start with {name}");
    }
}

#[test]
fn the_fixture_records_are_complete_enough_to_compare() {
    let fixture: Vec<Value> = serde_json::from_str(FIXTURE).unwrap();
    let mut ids: Vec<&str> = fixture
        .iter()
        .map(|record| record["oracle_id"].as_str().unwrap())
        .collect();
    let before = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), before, "oracle ids are unique");
    for record in &fixture {
        for field in ["id", "set", "collector_number", "mana_cost", "cmc"] {
            assert!(
                !record[field].is_null() || record["card_faces"].is_array(),
                "{}: no {field}",
                record["name"]
            );
        }
    }
}
