//! Goal: round 14 pack e14g reads the mana riders that give the creature spell the mana pays for counters or haste
//! (`AddsCounters$`, `AddsKeywords$`) into an enters bonus, and leaves a keyword that acts as the permanent enters, a
//! subtype exclusion and an uncounted number a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The uses of the mana of a land that taps for {C} with the extra keys.
fn uses(keys: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Land\nA:AB$ Mana | Cost$ T | Produced$ C | {keys}\n");
    let json = serde_json::to_value(abilities(&text)).unwrap();
    let only = json[0]["Activated"]["effects"][0]["AddMana"]["only"].clone();
    (serde_json::to_string(&only).unwrap(), gaps(&text))
}

#[rstest]
#[case::a_counter_on_a_creature(
    "AddsCounters$ Card.Creature_P1P1_1",
    r#"[{"Enters":{"counters":1,"spell":{"types":["Creature"]}}}]"#
)]
#[case::a_counter_on_a_multicolored_creature(
    "AddsCounters$ Card.Creature+MultiColor_P1P1_1",
    r#"[{"Enters":{"counters":1,"spell":{"multicolored":true,"types":["Creature"]}}}]"#
)]
#[case::haste_this_turn(
    "AddsKeywords$ Haste | AddsKeywordsValid$ Spell.Creature | AddsKeywordsUntil$ UntilEOT",
    r#"[{"Enters":{"keyword":"Haste","spell":{"types":["Creature"]},"this_turn":true}}]"#
)]
#[case::haste_for_good(
    "AddsKeywords$ Haste | AddsKeywordsValid$ Spell.Creature",
    r#"[{"Enters":{"keyword":"Haste","spell":{"types":["Creature"]}}}]"#
)]
#[case::haste_for_a_legend(
    "AddsKeywords$ Haste | AddsKeywordsValid$ Spell.Legendary+Creature | AddsKeywordsUntil$ UntilEOT",
    r#"[{"Enters":{"keyword":"Haste","spell":{"supertypes":["Legendary"],"types":["Creature"]},"this_turn":true}}]"#
)]
fn a_rider_on_the_creature_spell_is_an_enters_bonus(#[case] keys: &str, #[case] expected: &str) {
    let (only, found) = uses(keys);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(only, expected);
}

#[rstest]
#[case::riot(
    "AddsKeywords$ Riot | AddsKeywordsValid$ Spell.Creature",
    "key:Mana.AddsKeywords"
)]
#[case::non_human("AddsCounters$ Creature.nonHuman_P1P1_1", "key:Mana.AddsCounters")]
#[case::counted(
    "AddsCounters$ Card.YouOwn+IsCommander_P1P1_ManaAddsCounterNum",
    "key:Mana.AddsCounters"
)]
#[case::no_spells("AddsKeywords$ Haste", "key:Mana.AddsKeywordsValid")]
#[case::an_unknown_duration(
    "AddsKeywords$ Haste | AddsKeywordsValid$ Spell.Creature | AddsKeywordsUntil$ UntilYourNextTurn",
    "key:Mana.AddsKeywordsUntil"
)]
fn a_rider_the_engine_cannot_give_stays_a_gap(#[case] keys: &str, #[case] gap: &str) {
    let (_, found) = uses(keys);
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}
