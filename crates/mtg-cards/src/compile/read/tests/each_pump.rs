//! Goal: a pump of a group worded with "each" ("Each other creature you control gets +1/+1 until end of turn.") reads into the same effect as the plural wording ("Other creatures you control get +1/+1 until end of turn."), writes back as the text it came from, and the near-misses stay with the table.
use super::*;

#[rstest]
#[case::each_creature(
    "Each creature you control gets +1/+1 until end of turn.",
    ClauseKind::Spell
)]
#[case::each_other_creature(
    "Each other creature you control gets +2/+2 until end of turn.",
    ClauseKind::Spell
)]
#[case::keyword_after_the_stats(
    "Each creature you control gets +1/+1 and gains trample until end of turn.",
    ClauseKind::Spell
)]
#[case::only_a_keyword(
    "Each creature you control gains flying until end of turn.",
    ClauseKind::Spell
)]
#[case::a_creature_of_opponents(
    "Each creature your opponents control gets -1/-1 until end of turn.",
    ClauseKind::Spell
)]
#[case::in_an_attack_trigger(
    "Whenever ~ attacks, each other creature you control gets +1/+1 until end of turn.",
    ClauseKind::Triggered
)]
#[case::where_x_is_its_power(
    "Whenever ~ attacks, each other creature you control gets +X/+X until end of turn, where X is ~'s power.",
    ClauseKind::Triggered
)]
#[case::in_an_activation(
    "{2}{R}: Each creature you control gets +1/+0 until end of turn.",
    ClauseKind::Activated
)]
fn each_creature_gets_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind) {
    golden(text, kind, false);
}

#[rstest]
#[case::each_other_creature(
    "Each other creature you control gets +1/+1 until end of turn.",
    "Other creatures you control get +1/+1 until end of turn."
)]
#[case::each_creature(
    "Each creature you control gets +1/+1 and gains haste until end of turn.",
    "Creatures you control get +1/+1 and gain haste until end of turn."
)]
fn the_singular_and_the_plural_wording_are_one_effect(#[case] each: &str, #[case] plural: &str) {
    assert_eq!(spell(each), spell(plural));
    assert!(spell(each).is_some(), "{each}");
}

#[rstest]
#[case::the_plural_verb("Each creature you control get +1/+1 until end of turn.")]
#[case::the_singular_verb_for_a_plural_group("Creatures you control gets +1/+1 until end of turn.")]
#[case::each_with_two_targets("Each of two target creatures gets +1/+1 until end of turn.")]
#[case::counted_by_where_x(
    "Each creature you control gets +X/+X until end of turn, where X is its power."
)]
fn near_misses_of_a_pump_with_each_stay_with_the_table(#[case] text: &str) {
    assert!(spell_x(text).is_none(), "{text}");
}
