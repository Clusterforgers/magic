//! Goal: a trigger on casting a spell reads the kinds of spell cards name (a subtype, several subtypes, "permanent", "multicolored", "historic", a subtype with a card type), writes each back, and leaves near-misses to the table.
use super::*;

const PERMANENT: &str = r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}"#;

#[rstest]
#[case::a_subtype("a Merfolk spell", r#"{"Subtype":"Merfolk"}"#)]
#[case::an_article_before_a_vowel("an Elf spell", r#"{"Subtype":"Elf"}"#)]
#[case::two_subtypes(
    "an Aura or Equipment spell",
    r#"{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}"#
)]
#[case::three_subtypes(
    "an Aura, Equipment, or Vehicle spell",
    r#"{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"},{"Subtype":"Vehicle"}]}"#
)]
#[case::a_subtype_and_a_card_type(
    "a Vampire creature spell",
    r#"{"Type":"Creature"},{"Subtype":"Vampire"}"#
)]
#[case::permanent("a permanent spell", PERMANENT)]
#[case::subtypes_and_permanent(
    "a Faerie or Wizard permanent spell",
    r#"{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"AnyOf":[{"Subtype":"Faerie"},{"Subtype":"Wizard"}]}"#
)]
#[case::multicolored(
    "a multicolored spell",
    r#"{"Compare":{"stat":"Colors","op":"Ge","to":2}}"#
)]
#[case::historic(
    "a historic spell",
    r#"{"AnyOf":[{"Type":"Artifact"},{"Supertype":"Legendary"},{"Subtype":"Saga"}]}"#
)]
#[case::a_color("a black spell", r#"{"Color":"Black"}"#)]
fn the_spell_that_is_cast_has_the_kind(#[case] spell: &str, #[case] what: &str) {
    reads_as(
        &format!("Whenever you cast {spell}, draw a card."),
        ClauseKind::Triggered,
        false,
        &format!(
            r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[{what}],"by":"You"}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
        ),
    );
}

#[rstest]
#[case::an_opponent("an opponent casts", "Opponents")]
#[case::a_player("a player casts", "EachPlayer")]
fn another_player_casts_the_kind(#[case] who: &str, #[case] by: &str) {
    reads_as(
        &format!("Whenever {who} a multicolored permanent spell, draw a card."),
        ClauseKind::Triggered,
        false,
        &format!(
            r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[{{"Compare":{{"stat":"Colors","op":"Ge","to":2}}}},{PERMANENT}],"by":"{by}"}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
        ),
    );
}

#[rstest]
#[case::a_subtype_without_the_noun("Whenever you cast a Merfolk, draw a card.")]
#[case::an_adjective_and_a_subtype("Whenever you cast a noncreature or Dragon spell, draw a card.")]
#[case::a_name_that_is_no_subtype("Whenever you cast a Gizmo spell, draw a card.")]
#[case::a_subtype_after_a_type("Whenever you cast a creature Merfolk spell, draw a card.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
