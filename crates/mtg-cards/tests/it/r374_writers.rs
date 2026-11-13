//! Goal: the writers of the r37-4 pack word the counts of an object's own types and keyword values, the different mana
//! costs and color pairs, and nothing near them.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::{same_written_text, written_variants};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn writes(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_written_text(written, text))
}

/// Prints the wordings of each ability file in the directory `R374_ABILITIES`; a tool for writing rules files.
#[test]
fn probe() {
    let Ok(dir) = std::env::var("R374_ABILITIES") else {
        return;
    };
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        println!("== {}", path.display());
        for written in written_variants(&ability(&text)) {
            println!("  {written}");
        }
    }
}

/// Prints what the compiler reads of each line `kind|text` of the file `R374_CLAUSES`; a tool for writing readers.
#[test]
fn probe_clauses() {
    use mtg_cards::compile::{ClauseKind, w11d_room_clause};
    let Ok(file) = std::env::var("R374_CLAUSES") else {
        return;
    };
    for line in std::fs::read_to_string(file).unwrap().lines() {
        let Some((kind, text)) = line.split_once('|') else {
            continue;
        };
        let kind = match kind {
            "spell" => ClauseKind::Spell,
            "static" => ClauseKind::Static,
            "activated" => ClauseKind::Activated,
            _ => ClauseKind::Triggered,
        };
        let read = w11d_room_clause(text, kind);
        println!(
            "== {text}\n  {}",
            read.map_or("-".to_owned(), |a| serde_json::to_string(&a).unwrap())
        );
    }
}

fn each(amount: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}},"mods":[{{"ModifyPT":{{"power":{amount},"toughness":{amount}}}}}]}}}}"#
    )
}

const BUSHIDO: &str = r#"{"Stat":{"of":"Affected","stat":"R374Bushido"}}"#;
const CAPPED: &str = r#"{"R374AtMost":[{"Stat":{"of":"Affected","stat":"R374CreatureTypes"}},10]}"#;
const TYPES: &str = r#"{"Stat":{"of":"Affected","stat":"R374Types"}}"#;

#[rstest]
#[case::bushido(
    BUSHIDO,
    "Each creature you control gets +1/+1 for each point of bushido it has.",
    true
)]
#[case::bushido_is_no_toxic(
    BUSHIDO,
    "Each creature you control gets +1/+1 for each point of toxic it has.",
    false
)]
#[case::capped(
    CAPPED,
    "Each creature you control gets +1/+1 for each of its creature types, to a maximum of 10.",
    true
)]
#[case::another_cap(
    CAPPED,
    "Each creature you control gets +1/+1 for each of its creature types, to a maximum of 9.",
    false
)]
#[case::cap_left_out(
    CAPPED,
    "Each creature you control gets +1/+1 for each of its creature types.",
    false
)]
#[case::types(
    TYPES,
    "Each creature you control gets +1/+1 for each supertype, card type, and subtype it has.",
    true
)]
#[case::types_are_no_creature_types(
    TYPES,
    "Each creature you control gets +1/+1 for each of its creature types.",
    false
)]
fn r374_counts_of_its_own(#[case] amount: &str, #[case] text: &str, #[case] reads: bool) {
    assert_eq!(writes(&each(amount), text), reads);
}

fn gain(of: &str, stat: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"GainLife":{{"amount":{{"Distinct":{{"of":{of},"stat":"{stat}"}}}}}}}}]}}}}"#
    )
}

const PAIRS: &str = r#"[{"ControlledBy":"You"},{"Compare":{"stat":"Colors","op":"Eq","to":2}}]"#;
const COSTS: &str = r#"[{"Not":{"Type":"Land"}},{"In":"Graveyard"},{"OwnedBy":"You"}]"#;

#[rstest]
#[case::pairs(
    PAIRS,
    "R374ColorSet",
    "You gain life equal to the number of different color pairs among permanents you control that are exactly two colors.",
    true
)]
#[case::pairs_of_any_colors(
    r#"[{"ControlledBy":"You"}]"#,
    "R374ColorSet",
    "You gain life equal to the number of different color pairs among permanents you control.",
    false
)]
#[case::pairs_are_no_colors(
    PAIRS,
    "Colors",
    "You gain life equal to the number of different color pairs among permanents you control that are exactly two colors.",
    false
)]
#[case::mana_costs(
    COSTS,
    "R374ManaCost",
    "You gain life equal to the number of different mana costs among nonland cards in your graveyard.",
    true
)]
#[case::mana_costs_are_no_mana_values(
    COSTS,
    "R374ManaCost",
    "You gain life equal to the number of different mana values among nonland cards in your graveyard.",
    false
)]
fn r374_different_values(
    #[case] of: &str,
    #[case] stat: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&gain(of, stat), text), reads);
}

const HATCHERY: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"HighestCounters":{"who":"Opponents","counter":"poison"}},"op":"Ge","right":3}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"chosen"}},{"Draw":{"count":{"Stat":{"of":{"Bound":"chosen"},"stat":"R374Toxic"}}}}]}}"#;

#[rstest]
#[case::whole(
    "At the beginning of your upkeep, if an opponent has three or more poison counters, choose a creature you control, then draw cards equal to its total toxic value.",
    true
)]
#[case::another_number(
    "At the beginning of your upkeep, if an opponent has two or more poison counters, choose a creature you control, then draw cards equal to its total toxic value.",
    false
)]
#[case::the_stand_in(
    "At the beginning of your upkeep, if you control three or more artifacts, choose a creature you control, then draw cards equal to its total toxic value.",
    false
)]
#[case::its_power(
    "At the beginning of your upkeep, if an opponent has three or more poison counters, choose a creature you control, then draw cards equal to its power.",
    false
)]
fn r374_chosen_toxic(#[case] text: &str, #[case] reads: bool) {
    assert_eq!(writes(HATCHERY, text), reads);
}
