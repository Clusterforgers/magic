//! Goal: the replacements of the life a player would gain ("you gain that much life plus 1", "twice that much life"), the rules that stop it ("Players can't gain life.", "Your opponents can't gain life.") and the static abilities under "as long as you have 25 or more life" read into the shape of the hand-written files, write back as the same text, and leave every near-miss to the table.
use super::*;

#[rstest]
#[case::plus_one(
    "If you would gain life, you gain that much life plus 1 instead.",
    r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Add":1}}}"#
)]
#[case::plus_three(
    "If you would gain life, you gain that much life plus 3 instead.",
    r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Add":3}}}"#
)]
#[case::twice(
    "If you would gain life, you gain twice that much life instead.",
    r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Multiply":2}}}"#
)]
fn a_replacement_of_life_gain_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::every_player("Players can't gain life.", r#""EachPlayer""#)]
#[case::opponents("Your opponents can't gain life.", r#""Opponents""#)]
#[case::you("You can't gain life.", r#""You""#)]
fn a_rule_that_stops_life_gain_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] who: &str,
) {
    let json = format!(r#"{{"Rule":{{"rule":{{"CantGainLife":{who}}}}}}}"#);
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::more_life(
    "This creature gets +2/+2 as long as you have 25 or more life.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":25}}}}"#
)]
#[case::less_life(
    "This creature has flying as long as you have 5 or less life.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":5}}}}"#
)]
#[case::in_front(
    "As long as you have 25 or more life, creatures you control have lifelink.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Lifelink"}],"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":25}}}}"#
)]
fn a_static_ability_can_hold_while_you_have_a_life_total(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::an_opponent("If an opponent would gain life, you gain that much life plus 1 instead.")]
#[case::a_player("If a player would gain life, you gain that much life plus 1 instead.")]
#[case::three_times("If you would gain life, you gain three times that much life instead.")]
#[case::half("If you would gain life, you gain half that much life instead.")]
#[case::no_instead("If you would gain life, you gain that much life plus 1.")]
#[case::plus_nothing("If you would gain life, you gain that much life plus instead.")]
#[case::plus_zero("If you would gain life, you gain that much life plus 0 instead.")]
#[case::with_a_life_total(
    "If you would gain life while you have 10 or less life, you gain twice that much life instead."
)]
#[case::another_effect("If you would gain life, draw that many cards instead.")]
#[case::a_spell_wording("You gain that much life plus 1.")]
#[case::one_opponent("Target opponent can't gain life.")]
#[case::this_turn("Players can't gain life this turn.")]
#[case::life_loss("Players can't lose life.")]
#[case::the_game("You can't gain life for the rest of the game.")]
#[case::a_life_total_of_an_opponent(
    "This creature gets +2/+2 as long as an opponent has 25 or more life and a player has more."
)]
#[case::no_number("This creature gets +2/+2 as long as you have more life.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
