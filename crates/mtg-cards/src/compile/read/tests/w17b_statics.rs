//! Goal: the rules about untargetable graveyard cards, losing the game, casting during your turn, combat damage by
//! toughness and counters that remain read into the DSL and write back as the same text; another group, zone, turn or
//! characteristic stays with the table.
use super::*;

#[rstest]
#[case::graveyards(
    "Cards in graveyards can't be the targets of spells or abilities.",
    r#"{"Rule":{"rule":"GraveyardCardsUntargetable"}}"#
)]
#[case::cant_lose_and_cant_win(
    "You can't lose the game and your opponents can't win the game.",
    r#"{"Rule":{"rule":{"CantLoseGame":{"who":"You","opponents_cant_win":true}}}}"#
)]
#[case::cant_lose(
    "You can't lose the game.",
    r#"{"Rule":{"rule":{"CantLoseGame":{"who":"You"}}}}"#
)]
#[case::cant_cast_on_your_turn(
    "Your opponents can't cast spells during your turn.",
    r#"{"Rule":{"rule":{"CantCastSpells":{"who":"Opponents","your_turn":true}}}}"#
)]
#[case::toughness_of_yours(
    "Each creature you control assigns combat damage equal to its toughness rather than its power.",
    r#"{"Rule":{"rule":{"ToughnessAssignsDamage":"You"}}}"#
)]
#[case::toughness_of_all(
    "Each creature assigns combat damage equal to its toughness rather than its power.",
    r#"{"Rule":{"rule":{"ToughnessAssignsDamage":"EachPlayer"}}}"#
)]
#[case::counters_remain(
    "Counters remain on ~ as it moves to any zone other than a player's hand or library.",
    r#"{"Rule":{"rule":"CountersRemain"}}"#
)]
fn w17b_a_rule_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::exile("Cards in exile can't be the targets of spells or abilities.")]
#[case::only_spells("Cards in graveyards can't be the targets of spells.")]
#[case::opponents_cant_lose("Your opponents can't lose the game.")]
#[case::every_turn_of_yours("Your opponents can't cast spells during combat.")]
#[case::creature_spells("Your opponents can't cast creature spells during your turn.")]
#[case::power_by_toughness(
    "Each creature you control assigns combat damage equal to its power rather than its toughness."
)]
#[case::to_a_hand("Counters remain on ~ as it moves to any zone other than a player's library.")]
fn w17b_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
