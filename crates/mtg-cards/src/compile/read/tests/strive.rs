//! Goal: "Strive — This spell costs {2}{U} more to cast for each target beyond the first." reads as a cost increase counted by the targets (the ability word is flavor), "If you control a commander, you may cast this spell without paying its mana cost." as a conditional free cast, and "any number of target artifacts and/or enchantments" as a group of targets; each is written back as the same text and a near-miss stays with the table.
use super::*;

const BEYOND_THE_FIRST: &str = r#"{"Sum":[{"TargetCount":0},-1]}"#;

#[rstest]
#[case::generic("{1}", "{1}")]
#[case::colored("{2}{U}", "{2}{U}")]
#[case::two_colors("{1}{G}", "{1}{G}")]
fn strive_reads_with_its_ability_word_and_without_it(#[case] cost: &str, #[case] mana: &str) {
    let json = format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Increase","mana":"{mana}","per":{BEYOND_THE_FIRST}}}}}"#
    );
    let sentence =
        format!("This spell costs {cost} more to cast for each target beyond the first.");
    reads_as(&sentence, ClauseKind::Static, false, &json);
    assert_eq!(
        clause(
            &format!("Strive \u{2014} {sentence}"),
            ClauseKind::Static,
            false
        ),
        Some(ability(&json))
    );
}

#[rstest]
#[case::the_commander_free_cast(
    "If you control a commander, you may cast this spell without paying its mana cost.",
    r#"{"FreeCast":{"condition":{"Compare":{"left":{"Count":["Commander",{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}"#
)]
#[case::another_kind_of_permanent(
    "If you control a Forest, you may cast this spell without paying its mana cost.",
    r#"{"FreeCast":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Forest"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}"#
)]
fn the_conditional_free_cast_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::no_condition("You may cast this spell without paying its mana cost.")]
#[case::another_verb(
    "If you control a commander, you may play this spell without paying its mana cost."
)]
#[case::another_cost("If you control a commander, you may cast this spell by paying {1}.")]
#[case::strive_for_other_spells(
    "Strive \u{2014} Noncreature spells cost {1} more to cast for each target beyond the first."
)]
#[case::the_wrong_players(
    "If your opponents control a commander, you may cast this spell without paying its mana cost."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::artifacts_and_enchantments(
    "Destroy any number of target artifacts and/or enchantments.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]},"bounds":{"min":0,"max":null}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::creatures_and_planeswalkers(
    "Exile any number of target creatures and/or planeswalkers.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]},"bounds":{"min":0,"max":null}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#
)]
fn a_group_of_targets_is_joined_by_and_or(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn a_group_of_targets_is_not_joined_by_a_bare_or() {
    let text = "Destroy any number of target artifacts or enchantments.";
    assert_eq!(clause(text, ClauseKind::Spell, false), None);
}
