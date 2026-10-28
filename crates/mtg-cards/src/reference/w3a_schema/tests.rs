//! Goal: the schema names every type it uses, keeps to the keywords grammar converters read and to a size budget,
//! accepts every hand-written rules file and stored accepted proposal, and rejects the wrong shapes a local model wrote.
use std::fs;
use std::path::{Path, PathBuf};

use rstest::rstest;
use serde_json::Value;

use super::*;

/// The schema in bytes, compact, may not grow past this without a decision.
const W3A_BUDGET: usize = 120_000;

/// The keywords the schema may use: those the converters of llama.cpp and Ollama turn into a grammar.
const W3A_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "required",
    "additionalProperties",
    "enum",
    "items",
    "anyOf",
    "$ref",
    "$defs",
];

/// Checks `value` against `schema`, resolving `$ref` in `root`; the error names the path of the first mismatch.
fn w3a_check(root: &Value, schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(Value::String(target)) = schema.get("$ref") {
        let name = target.trim_start_matches("#/$defs/");
        let inner = root["$defs"]
            .get(name)
            .ok_or_else(|| format!("no definition {name}"))?;
        return w3a_check(root, inner, value, path);
    }
    if let Some(Value::Array(options)) = schema.get("anyOf") {
        let errors: Vec<String> = options
            .iter()
            .filter_map(|option| w3a_check(root, option, value, path).err())
            .collect();
        if errors.len() == options.len() {
            // The option that got furthest names the mistake.
            let deepest = errors
                .into_iter()
                .max_by_key(|error| error.split(": ").next().map_or(0, str::len));
            return Err(deepest.unwrap_or_default());
        }
        return Ok(());
    }
    if let Some(Value::Array(allowed)) = schema.get("enum")
        && !allowed.contains(value)
    {
        return Err(format!("{path}: {value} is not one of the names"));
    }
    w3a_check_type(root, schema, value, path)
}

fn w3a_check_type(root: &Value, schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let fits = match schema.get("type").and_then(Value::as_str) {
        None => true,
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("integer") => value.is_i64() || value.is_u64(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        Some(other) => return Err(format!("unknown type {other}")),
    };
    if !fits {
        return Err(format!("{path}: {value} is not a {}", schema["type"]));
    }
    if let (Some(items), Value::Array(list)) = (schema.get("items"), value) {
        for (at, item) in list.iter().enumerate() {
            w3a_check(root, items, item, &format!("{path}[{at}]"))?;
        }
    }
    if let Value::Object(map) = value {
        let properties = schema.get("properties").and_then(Value::as_object);
        for (key, inner) in map {
            match properties.and_then(|known| known.get(key)) {
                Some(property) => w3a_check(root, property, inner, &format!("{path}.{key}"))?,
                None if schema["additionalProperties"] == false => {
                    return Err(format!("{path}: {key} is not a field"));
                }
                None => {}
            }
        }
        for name in schema["required"].as_array().into_iter().flatten() {
            let name = name.as_str().unwrap_or_default();
            if !map.contains_key(name) {
                return Err(format!("{path}: {name} is missing"));
            }
        }
    }
    Ok(())
}

/// Checks the text of a file against the card schema; text that is not JSON fails too.
fn w3a_check_text(schema: &Value, text: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(text).map_err(|error| format!("not JSON: {error}"))?;
    w3a_check(schema, schema, &value, "")
}

fn w3a_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
}

fn w3a_dir(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Every keyword in the schema, object keys of `properties` and `$defs` aside.
fn w3a_keywords(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if !found.contains(key) {
                    found.push(key.clone());
                }
                match key.as_str() {
                    "properties" | "$defs" => inner
                        .as_object()
                        .into_iter()
                        .flat_map(|named| named.values())
                        .for_each(|each| w3a_keywords(each, found)),
                    _ => w3a_keywords(inner, found),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| w3a_keywords(item, found)),
        _ => {}
    }
}

#[test]
fn every_type_name_is_a_definition_or_a_primitive() {
    let (_, unknown) = w3a_build();
    assert!(unknown.is_empty(), "unknown types: {unknown:?}");
}

#[test]
fn the_schema_uses_only_converter_keywords_and_stays_under_its_budget() {
    let schema = w3a_card_schema();
    let mut found = Vec::new();
    w3a_keywords(&schema, &mut found);
    let extra: Vec<&String> = found
        .iter()
        .filter(|key| !W3A_KEYWORDS.contains(&key.as_str()))
        .collect();
    assert!(extra.is_empty(), "keywords outside the subset: {extra:?}");
    let size = serde_json::to_string(&schema).unwrap().len();
    assert!(
        size <= W3A_BUDGET,
        "the schema is {size} bytes, the budget {W3A_BUDGET}"
    );
}

#[test]
fn every_reference_points_at_a_definition() {
    let schema = w3a_card_schema();
    let text = serde_json::to_string(&schema).unwrap();
    for piece in text.split("\"#/$defs/").skip(1) {
        let name = piece.split('"').next().unwrap();
        assert!(schema["$defs"].get(name).is_some(), "no definition {name}");
    }
}

#[test]
fn every_hand_written_rules_file_fits_the_schema() {
    let schema = w3a_card_schema();
    let files = w3a_json_files(&w3a_dir("cards"));
    assert!(files.len() > 1_000, "only {} files", files.len());
    let failures: Vec<String> = files
        .iter()
        .filter_map(|file| {
            let text = fs::read_to_string(file).unwrap();
            w3a_check_text(&schema, &text)
                .err()
                .map(|error| format!("{}: {error}", file.display()))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} files do not fit:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Stored proposals that write a unit variant as `{"Commander": null}`: serde reads it, the schema keeps to the string.
const W3A_NULL_UNITS: &[&str] = &["crimson_honor_guard", "fight_for_the_throne"];

/// Whether serde reads every ability of the proposal.
fn w3a_serde_reads(text: &str) -> bool {
    let value: Value = serde_json::from_str(text).unwrap();
    value["faces"].as_array().unwrap().iter().all(|face| {
        face.get("abilities").is_none_or(|abilities| {
            serde_json::from_value::<Vec<crate::ability::Ability>>(abilities.clone()).is_ok()
        })
    })
}

#[test]
fn every_stored_proposal_fits_the_schema_exactly_when_serde_reads_it() {
    let schema = w3a_card_schema();
    let root = w3a_dir("tests/fixtures/proposals");
    let mut count = 0;
    let mut disagree = Vec::new();
    for round in fs::read_dir(&root).unwrap() {
        for file in w3a_json_files(&round.unwrap().path()) {
            count += 1;
            let slug = file.file_stem().unwrap().to_str().unwrap();
            let text = fs::read_to_string(&file).unwrap();
            let fits = w3a_check_text(&schema, &text);
            let reads = w3a_serde_reads(&text) && !W3A_NULL_UNITS.contains(&slug);
            if fits.is_ok() != reads {
                disagree.push(format!(
                    "{}: serde {reads}, schema {fits:?}",
                    file.display()
                ));
            }
        }
    }
    assert!(count > 1_000, "only {count} proposals");
    assert!(
        disagree.is_empty(),
        "{} disagree:\n{}",
        disagree.len(),
        disagree.join("\n")
    );
}

/// Wrong shapes a local model wrote: a unit keyword written as a struct one, a subtype among card types, an unknown
/// predicate, a JSON fragment as a predicate name, an array where a reference goes, a payload on a unit cost and two
/// abilities in one object.
#[rstest]
#[case::unit_for_a_struct_variant("frie_displaced_in_time", "AddKeyword")]
#[case::subtype_as_card_type("lush_growth", "SetTypes")]
#[case::unknown_predicate("sick_and_tired", "Each")]
#[case::json_fragment_as_a_name("sizzling_barrage", "Object[1]")]
#[case::array_for_a_reference("temple_altisaur", "Damage.to")]
#[case::payload_on_a_unit("the_general", "cost")]
#[case::two_keys_in_one_ability("thwip", "abilities[0]")]
fn wrong_shapes_are_rejected(#[case] slug: &str, #[case] place: &str) {
    let schema = w3a_card_schema();
    let text = fs::read_to_string(w3a_dir(&format!(
        "tests/fixtures/w3a_shapes/rejected/{slug}.json"
    )))
    .unwrap();
    let error = w3a_check_text(&schema, &text).unwrap_err();
    assert!(error.contains(place), "{slug}: {error}");
}

/// Wrong values in a valid shape, which only the checker finds: a player where an object is meant, an empty span.
#[rstest]
#[case::player_as_object("deep_sea_serpent")]
#[case::player_as_object_in_any("vexing_beetle")]
#[case::empty_span("festival_of_the_guildpact")]
fn wrong_values_in_a_valid_shape_pass(#[case] slug: &str) {
    let schema = w3a_card_schema();
    let text = fs::read_to_string(w3a_dir(&format!(
        "tests/fixtures/w3a_shapes/beyond/{slug}.json"
    )))
    .unwrap();
    assert_eq!(w3a_check_text(&schema, &text), Ok(()), "{slug}");
}
