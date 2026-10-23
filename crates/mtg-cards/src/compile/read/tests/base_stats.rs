//! Goal: "has base power and toughness N/N" in a static ability reads into `SetPT` (alone, or before the ability the permanent is given in quotes), for what an Aura or Equipment is attached to and for a group, writes back as the card's own text, and a size with a fraction, an X or a missing "base" stays with the table.
use super::*;

#[rstest]
#[case::an_aura(
    "Enchanted creature has base power and toughness 1/1.",
    r#"{"Static":{"mods":[{"SetPT":{"power":1,"toughness":1}}],"affected":"Attached"}}"#
)]
#[case::equipment(
    "Equipped creature has base power and toughness 4/4.",
    r#"{"Static":{"mods":[{"SetPT":{"power":4,"toughness":4}}],"affected":"Attached"}}"#
)]
#[case::a_group(
    "Creatures you control have base power and toughness 0/1.",
    r#"{"Static":{"mods":[{"SetPT":{"power":0,"toughness":1}}],"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#
)]
#[case::and_an_ability(
    "Enchanted creature has base power and toughness 8/8 and has \"Whenever this creature attacks, you may tap target creature with power 8 or less.\"",
    r#"{"Static":{"mods":[{"SetPT":{"power":8,"toughness":8}},{"GrantAbility":{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":8}}]}}],"effects":[{"May":{"effects":[{"Tap":{"what":{"Target":0}}}]}}]}}}],"affected":"Attached"}}"#
)]
fn a_base_size_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::no_base("Enchanted creature has power and toughness 1/1.")]
#[case::an_x("Enchanted creature has base power and toughness X/X.")]
#[case::a_fraction("Enchanted creature has base power and toughness 1/1 until end of turn.")]
#[case::only_power("Enchanted creature has base power 3.")]
fn a_size_the_template_does_not_model_is_left_to_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Static, false).is_none(), "{text}");
}
