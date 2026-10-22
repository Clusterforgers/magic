//! Goal: a group of permanents named by a subtype, a pair of card types or tokens ("All Sliver creatures get +1/+1", "Other Zombie creatures you control have swampwalk", "Creature tokens you control get +1/+1") reads into a static ability over the group, writes back as the same text, and leaves every near-miss to the table.
use super::*;

fn each(preds: &str) -> String {
    format!(r#"{{"Each":[{preds}]}}"#)
}

fn lord(preds: &str, mods: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{},"mods":[{mods}]}}}}"#,
        each(preds)
    )
}

const PLUS_ONE: &str = r#"{"ModifyPT":{"power":1,"toughness":1}}"#;
const CREATURE: &str = r#"{"Type":"Creature"}"#;
const YOURS: &str = r#"{"ControlledBy":"You"}"#;

#[rstest]
#[case::all_of_a_subtype(
    "All Sliver creatures get +1/+1.",
    &format!(r#"{CREATURE},{{"Subtype":"Sliver"}}"#),
    PLUS_ONE
)]
#[case::of_a_subtype_you_control(
    "Sliver creatures you control get +1/+1.",
    &format!(r#"{CREATURE},{{"Subtype":"Sliver"}},{YOURS}"#),
    PLUS_ONE
)]
#[case::a_subtype_with_a_keyword(
    "All Sliver creatures have flying.",
    &format!(r#"{CREATURE},{{"Subtype":"Sliver"}}"#),
    r#"{"AddKeyword":"Flying"}"#
)]
#[case::other_of_a_subtype(
    "Other Zombie creatures you control get +1/+1.",
    &format!(r#"{CREATURE},{{"Subtype":"Zombie"}},"Other",{YOURS}"#),
    PLUS_ONE
)]
#[case::the_plural_of_a_subtype(
    "Zombies you control get +1/+1.",
    &format!(r#"{{"Subtype":"Zombie"}},{YOURS}"#),
    PLUS_ONE
)]
#[case::two_types(
    "Artifact creatures you control get +1/+0.",
    &format!(r#"{{"Type":"Artifact"}},{CREATURE},{YOURS}"#),
    r#"{"ModifyPT":{"power":1,"toughness":0}}"#
)]
#[case::other_two_types(
    "Other artifact creatures you control have haste.",
    &format!(r#"{{"Type":"Artifact"}},{CREATURE},"Other",{YOURS}"#),
    r#"{"AddKeyword":"Haste"}"#
)]
#[case::tokens(
    "Creature tokens you control get +1/+1.",
    &format!(r#"{CREATURE},"Token",{YOURS}"#),
    PLUS_ONE
)]
#[case::tokens_alone(
    "Tokens you control get +1/+1.",
    &format!(r#""Token",{YOURS}"#),
    PLUS_ONE
)]
#[case::tokens_with_a_keyword(
    "Creature tokens you control have haste.",
    &format!(r#"{CREATURE},"Token",{YOURS}"#),
    r#"{"AddKeyword":"Haste"}"#
)]
#[case::nontoken_creatures_of_a_subtype(
    "Nontoken Elf creatures you control get +1/+1.",
    &format!(r#"{CREATURE},{{"Subtype":"Elf"}},{{"Not":"Token"}},{YOURS}"#),
    PLUS_ONE
)]
fn a_group_is_given_a_bonus_or_a_keyword(
    #[case] text: &str,
    #[case] group: &str,
    #[case] given: &str,
) {
    reads_as(text, ClauseKind::Static, false, &lord(group, given));
}

#[rstest]
#[case::destroy_all_of_a_subtype("Destroy all Goblin creatures.")]
#[case::a_spell_for_the_plural_of_a_subtype("Goblins you control get +1/+1 until end of turn.")]
#[case::a_spell_for_tokens("Creature tokens you control get +1/+1 until end of turn.")]
fn the_same_groups_read_in_a_spell(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::a_subtype_of_spells(
    "Dragon spells you cast cost {1} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Subtype":"Dragon"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}"}}"#
)]
#[case::two_less(
    "Villain spells you cast cost {2} less to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Subtype":"Villain"},{"ControlledBy":"You"}],"change":"Reduce","mana":"{2}"}}"#
)]
#[case::more_for_opponents(
    "Dragon spells your opponents cast cost {1} more to cast.",
    r#"{"CostModifier":{"applies_to":["Spell",{"Subtype":"Dragon"},{"ControlledBy":"Opponents"}],"change":"Increase","mana":"{1}"}}"#
)]
fn spells_of_a_subtype_cost_less_or_more(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::a_subtype_and_a_card("Dragon cards you cast cost {1} less to cast.")]
#[case::a_subtype_in_the_singular("Dragon spell you cast costs {1} less to cast.")]
#[case::a_cost_that_counts(
    "Dragon spells you cast cost {1} less to cast for each Dragon you control."
)]
fn near_misses_of_cost_modifiers_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::a_subtype_and_a_pair_of_types("Sliver artifact creatures get +1/+1.")]
#[case::two_subtypes("Sliver Zombie creatures get +1/+1.")]
#[case::a_qualifier_after_the_noun("Creatures Sliver get +1/+1.")]
#[case::an_unknown_subtype("Flumph creatures get +1/+1.")]
#[case::singular_tokens("Creature token you control gets +1/+1.")]
#[case::the_card_itself_gets_a_bonus("Sliver creatures get +1/+1 for each Sliver you control.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
