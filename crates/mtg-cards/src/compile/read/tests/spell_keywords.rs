//! Goal: a static ability that gives keywords to the spells a selector names ("Nonartifact spells you cast have improvise.", "Noncreature spells have convoke.") reads into a `Static` over `Spell`, writes back as the same text, and leaves every near-miss to the table.
use super::*;

fn given(spells: &str, keywords: &str) -> String {
    format!(r#"{{"Static":{{"affected":{{"Each":[{spells}]}},"mods":[{keywords}]}}}}"#)
}

const YOURS: &str = r#"{"ControlledBy":"You"}"#;

#[rstest]
#[case::nonartifact_spells_improvise(
    "Nonartifact spells you cast have improvise.",
    &format!(r#""Spell",{{"Not":{{"Type":"Artifact"}}}},{YOURS}"#),
    r#"{"AddKeyword":"Improvise"}"#
)]
#[case::artifact_spells_convoke(
    "Artifact spells you cast have convoke.",
    &format!(r#""Spell",{{"Type":"Artifact"}},{YOURS}"#),
    r#"{"AddKeyword":"Convoke"}"#
)]
#[case::legendary_creature_spells(
    "Legendary creature spells you cast have convoke.",
    &format!(r#""Spell",{{"Supertype":"Legendary"}},{{"Type":"Creature"}},{YOURS}"#),
    r#"{"AddKeyword":"Convoke"}"#
)]
#[case::enchantment_spells(
    "Enchantment spells you cast have convoke.",
    &format!(r#""Spell",{{"Type":"Enchantment"}},{YOURS}"#),
    r#"{"AddKeyword":"Convoke"}"#
)]
#[case::every_spell_you_cast(
    "Spells you cast have delve.",
    &format!(r#""Spell",{YOURS}"#),
    r#"{"AddKeyword":"Delve"}"#
)]
#[case::noncreature_spells_of_every_player(
    "Noncreature spells have convoke.",
    r#""Spell",{"Not":{"Type":"Creature"}}"#,
    r#"{"AddKeyword":"Convoke"}"#
)]
#[case::spells_of_your_opponents(
    "Artifact spells your opponents cast have improvise.",
    r#""Spell",{"Type":"Artifact"},{"ControlledBy":"Opponents"}"#,
    r#"{"AddKeyword":"Improvise"}"#
)]
fn spells_are_given_a_keyword(#[case] text: &str, #[case] spells: &str, #[case] keywords: &str) {
    reads_as(text, ClauseKind::Static, false, &given(spells, keywords));
}

#[rstest]
#[case::a_singular_spell("Nonartifact spell you cast has improvise.")]
#[case::cards_not_spells("Nonartifact cards you cast have improvise.")]
#[case::a_number("Nonartifact spells you cast have improvise 2.")]
#[case::without_the_keyword("Nonartifact spells you cast have.")]
#[case::a_keyword_a_spell_does_not_use("Nonartifact spells you cast have flying.")]
#[case::a_pump_not_a_keyword("Nonartifact spells you cast get +1/+1.")]
#[case::a_condition_after(
    "Nonartifact spells you cast have improvise as long as you control a creature."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
