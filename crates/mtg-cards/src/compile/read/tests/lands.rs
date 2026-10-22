//! Goal: the extra land plays of a turn and the permissions to play lands from a graveyard or the top of a library read into the rules of the hand-written files, write back as the same text, and leave every near-miss to the table.
use super::*;

#[rstest]
#[case::one_more(
    "You may play an additional land on each of your turns.",
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{}}}}"#
)]
#[case::two_more(
    "You may play two additional lands on each of your turns.",
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"count":2}}}}"#
)]
#[case::three_more(
    "You may play three additional lands on each of your turns.",
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"count":3}}}}"#
)]
#[case::every_player(
    "Each player may play an additional land on each of their turns.",
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"who":"EachPlayer"}}}}"#
)]
#[case::every_player_during(
    "Each player may play an additional land during each of their turns.",
    r#"{"Rule":{"rule":{"AdditionalLandPlays":{"who":"EachPlayer"}}}}"#
)]
#[case::from_a_graveyard(
    "You may play lands from your graveyard.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Land"}],"from":"Graveyard","play":true}}}}"#
)]
#[case::from_the_top(
    "You may play lands from the top of your library.",
    r#"{"Rule":{"rule":{"MayCast":{"what":[{"Type":"Land"}],"from":"Library","play":true}}}}"#
)]
#[case::looking_at_the_top(
    "You may look at the top card of your library any time.",
    r#"{"Rule":{"rule":{"LookAtTopAnyTime":"You"}}}"#
)]
fn a_static_rule_about_lands_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::an_additional_land(
    "You may play an additional land this turn.",
    r#"{"Spell":{"effects":[{"Rule":{"rule":{"AdditionalLandPlays":{}},"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::up_to_three_lands(
    "You may play up to three additional lands this turn.",
    r#"{"Spell":{"effects":[{"Rule":{"rule":{"AdditionalLandPlays":{"count":3}},"duration":"UntilEndOfTurn"}}]}}"#
)]
fn extra_land_plays_of_a_turn_are_an_effect(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::no_additional("You may play a land on each of your turns.")]
#[case::no_turn("You may play an additional land.")]
#[case::plural_for_one("You may play an additional lands on each of your turns.")]
#[case::singular_for_two("You may play two additional land on each of your turns.")]
#[case::your_turn_only("You may play an additional land on each turn.")]
#[case::lands_from_your_hand("You may play lands from your hand.")]
#[case::one_land("You may play a land from your graveyard.")]
#[case::their_graveyards("You may play lands from your opponents' graveyards.")]
#[case::lands_and_spells("You may play lands and cast spells from the top of your library.")]
#[case::until_end_of_turn("You may play lands from your graveyard this turn.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[test]
fn an_effect_of_extra_land_plays_needs_its_turn() {
    assert_eq!(
        clause("You may play an additional land.", ClauseKind::Spell, false),
        None
    );
}
