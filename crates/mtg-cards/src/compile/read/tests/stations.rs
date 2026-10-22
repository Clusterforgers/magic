//! Goal: a station block ("9+ | Flying", the lines under it) reads into abilities conditioned on the charge counters, whatever its threshold and lines, the block of the highest threshold also makes a Spacecraft with a printed size a creature, and every near-miss stays with the table.
use super::*;

use crate::compile::read::station_block;

/// The condition of a threshold of charge counters, as data.
fn charged(threshold: u32) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Counters":{{"on":"This","counter":"charge"}}}},"op":"Ge","right":{threshold}}}}}"#
    )
}

fn keywords(threshold: u32, words: &[&str]) -> String {
    let mods: Vec<String> = words
        .iter()
        .map(|word| format!(r#"{{"AddKeyword":"{word}"}}"#))
        .collect();
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{}],"condition":{}}}}}"#,
        mods.join(","),
        charged(threshold)
    )
}

fn creature(threshold: u32) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"AddType":"Creature"}}],"condition":{}}}}}"#,
        charged(threshold)
    )
}

fn all_of(abilities: &[String]) -> Vec<Ability> {
    abilities.iter().map(|json| ability(json)).collect()
}

/// The block read on a Spacecraft that becomes a creature at `creature_at` charge counters, if it does.
fn block(text: &str, creature_at: Option<i32>) -> Option<Vec<Ability>> {
    let own = Own {
        creature_at,
        ..Own::default()
    };
    station_block(text, false, &own)
}

#[rstest]
#[case::one("1+ | Flying", 1, &["Flying"])]
#[case::a_few("3+ | Flying, haste", 3, &["Flying", "Haste"])]
#[case::two_digits("12+ | Flying, lifelink", 12, &["Flying", "Lifelink"])]
#[case::twenty("20+ | Trample", 20, &["Trample"])]
fn keywords_after_the_bar_are_given_while_the_charge_counters_reach_the_threshold(
    #[case] text: &str,
    #[case] threshold: u32,
    #[case] words: &[&str],
) {
    assert_eq!(
        block(text, None),
        Some(all_of(&[keywords(threshold, words)])),
        "{text}"
    );
}

#[test]
fn the_highest_threshold_makes_a_spacecraft_with_a_size_a_creature() {
    assert_eq!(
        block("8+ | Flying", Some(8)),
        Some(all_of(&[creature(8), keywords(8, &["Flying"])]))
    );
    assert_eq!(
        block("2+ | Flying", Some(8)),
        Some(all_of(&[keywords(2, &["Flying"])]))
    );
}

#[test]
fn the_lines_under_the_first_belong_to_the_block() {
    let when = charged(7);
    let Some(abilities) = block(
        "7+ | Flying\nWhenever ~ attacks, draw a card.\n{1}{R}: ~ gets +2/+0 until end of turn.",
        Some(7),
    ) else {
        panic!("the block was not read");
    };
    assert_eq!(abilities.len(), 4);
    let condition: crate::value::Condition = serde_json::from_str(&when).unwrap();
    assert!(matches!(
        &abilities[2],
        Ability::Triggered { intervening_if: Some(found), .. } if *found == condition
    ));
    assert!(matches!(
        &abilities[3],
        Ability::Activated { condition: Some(found), .. } if *found == condition
    ));
}

#[test]
fn a_static_ability_keeps_its_own_condition_beside_the_charge_counters() {
    let Some(abilities) = block("2+ | ~ has haste as long as it's your turn.", None) else {
        panic!("the block was not read");
    };
    let Some(Ability::Static {
        condition: Some(crate::value::Condition::All(both)),
        ..
    }) = abilities.first()
    else {
        panic!("{abilities:?}");
    };
    assert_eq!(
        both.first(),
        Some(&serde_json::from_str(&charged(2)).unwrap())
    );
    assert_eq!(both.get(1), Some(&crate::value::Condition::YourTurn));
}

#[rstest]
#[case::no_bar("8+ Flying")]
#[case::no_plus("8 | Flying")]
#[case::no_number("+ | Flying")]
#[case::a_word_for_the_number("many+ | Flying")]
#[case::a_level_header("LEVEL 2-3\n3/3")]
#[case::nothing_after_the_bar("8+ | ")]
#[case::an_unread_line("8+ | Flying\nFrobnicate the widgets.")]
#[case::a_spell_line("8+ | Draw a card.")]
fn a_block_the_reader_does_not_know_stays_with_the_table(#[case] text: &str) {
    assert_eq!(block(text, Some(8)), None, "{text}");
}
