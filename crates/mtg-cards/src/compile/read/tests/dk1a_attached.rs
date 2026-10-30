//! Goal: a static that depends on who controls the enchanted creature reads into its condition, a second sentence "Otherwise, this Aura has K." reads as the same condition negated, both write back as the printed text, and a sentence of another shape stays unread.
use super::*;
use crate::compile::read::{Own, same_text};
use crate::compile::read::{static_pair, static_pair_texts};

/// "Another player controls enchanted creature" as a condition.
const ANOTHER: &str =
    r#"{"Matches":{"subject":"Attached","selector":[{"Not":{"ControlledBy":"You"}}]}}"#;

#[rstest]
#[case::another_player(
    "As long as another player controls enchanted creature, it can't attack or block.",
    ANOTHER
)]
#[case::an_opponent(
    "As long as an opponent controls enchanted creature, it can't attack or block.",
    r#"{"Matches":{"subject":"Attached","selector":[{"ControlledBy":"Opponents"}]}}"#
)]
#[case::you(
    "As long as you control enchanted creature, it can't attack or block.",
    r#"{"Matches":{"subject":"Attached","selector":[{"ControlledBy":"You"}]}}"#
)]
#[case::equipped(
    "As long as another player controls equipped creature, it can't attack or block.",
    ANOTHER
)]
fn a_condition_on_the_controller_of_the_attached_creature_reads_and_writes_back(
    #[case] text: &str,
    #[case] condition: &str,
) {
    let read = golden(text, ClauseKind::Static, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(condition), "{json}");
    assert!(json.contains(r#""affected":"Attached""#), "{json}");
    assert!(
        json.contains(r#""mods":["CantAttack","CantBlock"]"#),
        "{json}"
    );
}

#[rstest]
#[case::cant_block(
    "As long as another player controls enchanted creature, it can't block.",
    r#""mods":["CantBlock"]"#
)]
#[case::cant_attack(
    "As long as another player controls enchanted creature, it can't attack.",
    r#""mods":["CantAttack"]"#
)]
#[case::leading_subject(
    "As long as another player controls enchanted creature, enchanted creature can't attack or block.",
    r#""mods":["CantAttack","CantBlock"]"#
)]
fn the_restriction_follows_the_condition(#[case] text: &str, #[case] mods: &str) {
    let read =
        clause(text, ClauseKind::Static, false).unwrap_or_else(|| panic!("not read: {text}"));
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(mods), "{json}");
    assert!(json.contains(ANOTHER), "{json}");
}

fn pair(text: &str) -> Option<Vec<Ability>> {
    static_pair(text, false, &Own::default())
}

#[rstest]
#[case::umbra_armor(
    "As long as another player controls enchanted creature, it can't attack or block. Otherwise, this Aura has umbra armor.",
    r#"[{"AddKeyword":"UmbraArmor"}]"#
)]
#[case::flying(
    "As long as another player controls enchanted creature, it can't block. Otherwise, this Aura has flying.",
    r#"[{"AddKeyword":"Flying"}]"#
)]
fn otherwise_is_the_opposite_condition(#[case] text: &str, #[case] mods: &str) {
    let read = pair(text).unwrap_or_else(|| panic!("not read: {text}"));
    let [first, second] = read.as_slice() else {
        panic!("{read:?}");
    };
    assert_eq!(
        serde_json::to_value(second).unwrap(),
        serde_json::from_str::<serde_json::Value>(&format!(
            r#"{{"Static":{{"affected":"This","mods":{mods},"condition":{{"Not":{ANOTHER}}}}}}}"#
        ))
        .unwrap(),
        "{text}"
    );
    let written = static_pair_texts(first, second);
    assert!(
        written.iter().any(|variant| same_text(variant, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::no_otherwise(
    "As long as another player controls enchanted creature, it can't attack or block."
)]
#[case::an_effect_after(
    "As long as another player controls enchanted creature, it can't attack or block. Otherwise, draw a card."
)]
#[case::the_card_is_not_the_aura(
    "As long as another player controls enchanted creature, it can't attack or block. Otherwise, enchanted creature has flying."
)]
#[case::an_unconditional_first_sentence(
    "Enchanted creature can't attack or block. Otherwise, this Aura has umbra armor."
)]
fn another_shape_is_not_a_pair(#[case] text: &str) {
    assert_eq!(pair(text), None, "{text}");
}

#[test]
fn a_pair_of_other_abilities_writes_nothing() {
    let first = ability(r#"{"Static":{"affected":"Attached","mods":["CantBlock"]}}"#);
    let second = ability(
        r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":{"Not":{"Matches":{"subject":"Attached","selector":["Untapped"]}}}}}"#,
    );
    assert!(static_pair_texts(&first, &second).is_empty());
}
