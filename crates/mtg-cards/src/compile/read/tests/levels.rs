//! Goal: a level block ("LEVEL 2-3", the power and toughness, the abilities) reads into abilities conditioned on the level counters, whatever its range, size and lines; "each creature you control with a +1/+1 counter on it" and "attacks while saddled" read and write back as the card words them, and every near-miss stays with the table.
use super::*;

use crate::compile::read::level_block;
use crate::compile::{block_plan, without_header};
use crate::value::Condition;

/// The condition of a range of level counters, as data.
fn between(least: u32, most: u32) -> String {
    format!(
        r#"{{"All":[{},{}]}}"#,
        compare("Ge", least),
        compare("Le", most)
    )
}

fn compare(op: &str, number: u32) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Counters":{{"on":"This","counter":{{"Other":"level"}}}}}},"op":"{op}","right":{number}}}}}"#
    )
}

fn size(condition: &str, power: u32, toughness: u32) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetPT":{{"power":{power},"toughness":{toughness}}}}}],"condition":{condition}}}}}"#
    )
}

fn block(text: &str) -> Option<Vec<Ability>> {
    level_block(text, false, &Own::default())
}

fn all_of(abilities: &[String]) -> Vec<Ability> {
    abilities.iter().map(|json| ability(json)).collect()
}

#[rstest]
#[case::a_range("LEVEL 2-3\n3/3", between(2, 3), 3, 3)]
#[case::a_single_level("LEVEL 5-5\n1/4", between(5, 5), 1, 4)]
#[case::no_end("LEVEL 4+\n4/4", compare("Ge", 4), 4, 4)]
#[case::from_one("LEVEL 1+\n0/9", compare("Ge", 1), 0, 9)]
#[case::two_digits("LEVEL 10-12\n12/12", between(10, 12), 12, 12)]
fn a_header_and_a_size_set_the_power_and_toughness_while_the_levels_are_in_range(
    #[case] text: &str,
    #[case] condition: String,
    #[case] power: u32,
    #[case] toughness: u32,
) {
    assert_eq!(
        block(text),
        Some(all_of(&[size(&condition, power, toughness)])),
        "{text}"
    );
}

#[test]
fn keywords_under_a_header_are_given_while_the_levels_are_in_range() {
    let when = between(2, 3);
    assert_eq!(
        block("LEVEL 2-3\n3/3\nFlying\nFirst strike, lifelink"),
        Some(all_of(&[
            size(&when, 3, 3),
            format!(
                r#"{{"Static":{{"affected":"This","mods":[{{"AddKeyword":"Flying"}}],"condition":{when}}}}}"#
            ),
            format!(
                r#"{{"Static":{{"affected":"This","mods":[{{"AddKeyword":"FirstStrike"}},{{"AddKeyword":"Lifelink"}}],"condition":{when}}}}}"#
            ),
        ]))
    );
}

#[test]
fn an_activated_ability_under_a_header_is_activated_only_while_the_levels_are_in_range() {
    let when = compare("Ge", 6);
    assert_eq!(
        block("LEVEL 6+\n1/1\n{T}: Create two 3/3 green Elephant creature tokens."),
        Some(all_of(&[
            size(&when, 1, 1),
            format!(
                r#"{{"Activated":{{"cost":["Tap"],"condition":{when},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Elephant","colors":["Green"],"types":["Creature"],"subtypes":["Elephant"],"power":3,"toughness":3}}}},"count":2}}}}]}}}}"#
            ),
        ]))
    );
}

#[test]
fn a_static_ability_keeps_its_own_condition_beside_the_levels() {
    let when = compare("Ge", 2);
    let Some(abilities) = block("LEVEL 2+\n2/2\n~ has haste as long as it's your turn.") else {
        panic!("the block was not read");
    };
    let Some(Ability::Static {
        condition: Some(Condition::All(both)),
        ..
    }) = abilities.get(1)
    else {
        panic!("{abilities:?}");
    };
    assert_eq!(both.len(), 2);
    assert_eq!(both.first(), Some(&serde_json::from_str(&when).unwrap()));
    assert_eq!(both.get(1), Some(&Condition::YourTurn));
}

#[rstest]
#[case::no_size("LEVEL 2-3\nFlying")]
#[case::no_lines("LEVEL 2-3")]
#[case::a_range_backwards("LEVEL 3-2\n1/1")]
#[case::no_number("LEVEL +\n1/1")]
#[case::a_header_that_is_not_one("Level up {1}\n1/1")]
#[case::an_unread_line("LEVEL 2-3\n1/1\nFrobnicate the widgets.")]
#[case::a_size_that_is_not_one("LEVEL 2-3\n*/1")]
#[case::a_spell_line_in_the_block("LEVEL 2-3\n1/1\nDraw a card.")]
#[case::an_ability_granted_that_is_unread(
    "LEVEL 5+\n1/4\nElves you control have \"{T}: Do something odd.\""
)]
fn a_block_with_a_line_the_reader_does_not_know_stays_with_the_table(#[case] text: &str) {
    assert_eq!(block(text), None, "{text}");
}

#[rstest]
#[case::flying(
    "Each creature you control with a +1/+1 counter on it has flying.",
    r#"{"HasCounter":"+1/+1"}"#,
    r#"{"AddKeyword":"Flying"}"#
)]
#[case::first_strike(
    "Each creature you control with a +1/+1 counter on it has first strike.",
    r#"{"HasCounter":"+1/+1"}"#,
    r#"{"AddKeyword":"FirstStrike"}"#
)]
#[case::a_pump(
    "Each creature you control with a -1/-1 counter on it gets +2/+0.",
    r#"{"HasCounter":"-1/-1"}"#,
    r#"{"ModifyPT":{"power":2,"toughness":0}}"#
)]
#[case::a_named_counter(
    "Each creature you control with a charge counter on it has haste.",
    r#"{"HasCounter":"charge"}"#,
    r#"{"AddKeyword":"Haste"}"#
)]
#[case::an_article_before_a_vowel(
    "Each creature you control with an age counter on it has haste.",
    r#"{"HasCounter":{"Other":"age"}}"#,
    r#"{"AddKeyword":"Haste"}"#
)]
fn each_creature_with_a_counter_on_it_is_given_a_bonus_or_a_keyword(
    #[case] text: &str,
    #[case] counter: &str,
    #[case] given: &str,
) {
    let json = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},{counter}]}},"mods":[{given}]}}}}"#
    );
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::the_plural_form("Creatures you control with a +1/+1 counter on it have flying.")]
#[case::no_article("Each creature you control with +1/+1 counter on it has flying.")]
#[case::on_another_object("Each creature you control with a +1/+1 counter on ~ has flying.")]
#[case::a_rules_counter("Each creature you control with a flying counter on it has haste.")]
#[case::a_verb_for_many("Each creature you control with a +1/+1 counter on it have flying.")]
fn near_misses_of_a_group_with_a_counter_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::a_token(
    "Whenever ~ attacks while saddled, create a 1/1 white Sheep creature token.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Matches":{"subject":"This","selector":[{"HasKeyword":"Saddled"}]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Sheep","colors":["White"],"types":["Creature"],"subtypes":["Sheep"],"power":1,"toughness":1}}}}]}}"#
)]
#[case::a_draw(
    "Whenever ~ attacks while saddled, draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Matches":{"subject":"This","selector":[{"HasKeyword":"Saddled"}]}},"effects":[{"Draw":{}}]}}"#
)]
fn attacking_while_saddled_is_the_attack_with_the_saddled_designation(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::another_event("Whenever ~ enters while saddled, draw a card.")]
#[case::with_a_life_condition(
    "Whenever ~ attacks while saddled, if you have 5 or less life, draw a card."
)]
fn saddled_is_read_only_for_the_attack(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[rstest]
#[case::a_leveler("LEVEL 2-3\n2/4\nFlying\n{T}: Draw a card.", None, 1, &["Flying", "{T}: Draw a card."])]
#[case::a_class("{3}{U}: Level 2\nWhen this Class becomes level 2, draw a card.", None, 1, &["When this Class becomes level 2, draw a card."])]
#[case::a_class_with_several_lines("{1}{R}: Level 3\nFlying\nHaste", None, 1, &["Flying", "Haste"])]
#[case::a_station_block("3+ | Flying, haste\nWhen this Spacecraft enters, draw a card.", None, 0, &["Flying, haste", "When this Spacecraft enters, draw a card."])]
#[case::the_station_where_it_becomes_a_creature("8+ | Flying", Some(8), 1, &["Flying"])]
#[case::another_station_than_the_creature_one("3+ | Flying", Some(8), 0, &["Flying"])]
fn a_plan_splits_a_block_into_the_abilities_of_its_header_and_the_lines_under_it(
    #[case] text: &str,
    #[case] creature_at: Option<i32>,
    #[case] head: usize,
    #[case] lines: &[&str],
) {
    let Some(plan) = block_plan(text, creature_at) else {
        panic!("no plan: {text}");
    };

    assert_eq!(plan.head.len(), head, "{text}");
    assert_eq!(plan.lines, lines, "{text}");
}

#[rstest]
#[case::flying("Flying")]
#[case::level_up("Level up {1}{U}")]
#[case::a_class_header_without_cost("Level 2")]
#[case::a_range_backwards("LEVEL 3-2\n1/1")]
#[case::a_leveler_without_a_size("LEVEL 2-3\nFlying")]
#[case::a_station_without_a_threshold("+ | Flying")]
#[case::a_threshold_that_is_not_a_number("x+ | Flying")]
fn a_text_that_is_no_block_has_no_plan(#[case] text: &str) {
    assert_eq!(block_plan(text, None), None, "{text}");
}

/// A JSON text with the places of the condition filled: `RANGE` is the whole range, `GE` and `LE` its two ends.
fn with_range(json: &str) -> String {
    json.replace("RANGE", &between(2, 3))
        .replace("GE", &compare("Ge", 2))
        .replace("LE", &compare("Le", 3))
}

#[rstest]
#[case::the_levels_alone(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":RANGE}}"#,
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}]}}"#
)]
#[case::an_own_condition_after_it(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":{"All":[RANGE,"YourTurn"]}}}"#,
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":"YourTurn"}}"#
)]
#[case::an_own_condition_before_it(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":{"All":["YourTurn",RANGE]}}}"#,
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":"YourTurn"}}"#
)]
#[case::both_ends_beside_an_own_condition(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":{"All":[GE,LE,"YourTurn"]}}}"#,
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":"YourTurn"}}"#
)]
#[case::an_activation(
    r#"{"Activated":{"cost":["Tap"],"condition":RANGE,"effects":[{"Draw":{}}]}}"#,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#
)]
#[case::a_trigger(
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":RANGE,"effects":[{"Draw":{}}]}}"#,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Draw":{}}]}}"#
)]
fn the_condition_of_a_block_is_taken_off_an_ability_that_works_under_it(
    #[case] with: &str,
    #[case] without: &str,
) {
    let level: Condition = serde_json::from_str(&between(2, 3)).unwrap();

    assert_eq!(
        without_header(&ability(&with_range(with)), &level),
        Some(ability(&with_range(without)))
    );
}

#[rstest]
#[case::no_condition(r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}]}}"#)]
#[case::another_range(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":{"All":[GE,{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"level"}}},"op":"Le","right":4}}]}}}"#
)]
#[case::only_one_end(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":GE}}"#
)]
#[case::another_condition(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":"YourTurn"}}"#
)]
#[case::a_spell_has_no_condition(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#)]
fn an_ability_that_does_not_work_under_the_condition_of_a_block_keeps_none_of_it(
    #[case] json: &str,
) {
    let level: Condition = serde_json::from_str(&between(2, 3)).unwrap();

    assert_eq!(
        without_header(&ability(&with_range(json)), &level),
        None,
        "{json}"
    );
}
