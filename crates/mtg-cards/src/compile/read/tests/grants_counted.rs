//! Goal: the statics that count what they give by a set of kinds ("artifact and/or enchantment"), by the opponents a player has, that take a keyword away beside a bonus, or that give protection from what the card chose or from a creature type read into the shapes of the hand-written files and write back as the card's own text.
use super::*;

#[rstest]
#[case::artifact_and_or_enchantment(
    "Equipped creature gets +1/+1 for each artifact and/or enchantment you control.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"ControlledBy":"You"}]},"toughness":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::artifact_or_enchantment_written_the_other_way(
    "Enchanted creature gets +1/+1 for each artifact or enchantment you control.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"ControlledBy":"You"}]},"toughness":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::for_each_opponent_you_have(
    "Equipped creature gets +1/+0 for each opponent you have.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Players":"Opponents"},"toughness":0}}]}}"#
)]
#[case::a_keyword_lost_beside_a_bonus(
    "Equipped creature gets +10/+10 and loses flying.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":10,"toughness":10}},{"RemoveKeyword":"Flying"}]}}"#
)]
#[case::protection_from_the_chosen_color(
    "This creature has protection from the chosen color.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":{"Protection":["ChosenColor"]}}]}}"#
)]
#[case::protection_from_the_chosen_type(
    "Human creatures you control have protection from creatures of the chosen type.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Subtype":"Human"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Protection":[{"Type":"Creature"},"ChosenSubtype"]}}]}}"#
)]
#[case::protection_from_a_creature_type(
    "Cats you control have protection from Rats.",
    r#"{"Static":{"affected":{"Each":[{"Subtype":"Cat"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Protection":[{"Subtype":"Rat"}]}}]}}"#
)]
fn a_counted_or_chosen_static_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::the_chosen_color_with_a_bonus_on_the_card("This creature gets +1/+1 and has protection from the chosen color.")]
#[case::protection_from_a_noun_that_is_no_subtype("Cats you control have protection from Gizmos.")]
#[case::a_keyword_lost_that_is_no_keyword("Equipped creature gets +1/+1 and loses sparkles.")]
#[case::the_opponents_with_another_tail("Equipped creature gets +1/+0 for each opponent you hate.")]
fn near_misses_are_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
