//! Goal: the sentence that limits when an ability is activated reads into one timing, limit and condition and writes back as the card words it: a moment of the turn ("during your upkeep", "during your turn, before attackers are declared", "during combat", "during the end of combat step"), "if" and a state of the game, "as a sorcery" and "once each turn", joined by "and only" in any order; every other moment, order or condition stays with the table.
use super::*;
use crate::ability::{Limit, Timing};
use crate::trigger::Step;
use crate::value::Condition;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

/// The timing, limit and condition an activated ability reads with.
fn limits(text: &str) -> (Timing, Option<Limit>, Option<Condition>) {
    match golden(text, ClauseKind::Activated, false) {
        Ability::Activated {
            timing,
            limit,
            condition,
            ..
        } => (timing, limit, condition),
        other => panic!("not an activated ability: {other:?}"),
    }
}

#[rstest]
#[case::your_turn(
    "{2}: Draw a card. Activate only during your turn.",
    Some(Condition::YourTurn)
)]
#[case::your_upkeep(
    "{3}{W}{W}: Return this card from your graveyard to your hand. Activate only during your upkeep.",
    Some(Condition::All(vec![Condition::YourTurn, Condition::Step(Step::Upkeep)]))
)]
#[case::an_opponents_turn(
    "{2}: Draw a card. Activate only during an opponent's turn.",
    Some(Condition::Not(Box::new(Condition::YourTurn)))
)]
#[case::an_opponents_upkeep(
    "{2}: Draw a card. Activate only during an opponent's upkeep.",
    Some(Condition::All(vec![Condition::Not(Box::new(Condition::YourTurn)), Condition::Step(Step::Upkeep)]))
)]
#[case::any_upkeep(
    "{2}: Draw a card. Activate only during any upkeep step.",
    Some(Condition::Step(Step::Upkeep))
)]
#[case::before_attackers(
    "{2}: Draw a card. Activate only before attackers are declared.",
    Some(Condition::BeforeAttackers)
)]
#[case::your_turn_before_attackers(
    "{T}: Put target creature card from your graveyard on top of your library. Activate only during your turn, before attackers are declared.",
    Some(Condition::All(vec![Condition::YourTurn, Condition::BeforeAttackers]))
)]
#[case::an_opponents_turn_before_attackers(
    "{2}: Draw a card. Activate only during an opponent's turn, before attackers are declared.",
    Some(Condition::All(vec![Condition::Not(Box::new(Condition::YourTurn)), Condition::BeforeAttackers]))
)]
#[case::combat(
    "{2}: Draw a card. Activate only during combat.",
    Some(Condition::InCombat)
)]
#[case::declare_blockers(
    "{2}: Draw a card. Activate only during the declare blockers step.",
    Some(Condition::Step(Step::DeclareBlockers))
)]
#[case::declare_attackers(
    "{2}: Draw a card. Activate only during the declare attackers step.",
    Some(Condition::Step(Step::DeclareAttackers))
)]
#[case::end_of_combat(
    "{T}: This land deals 1 damage to target attacking creature. Activate only during the end of combat step.",
    Some(Condition::Step(Step::EndCombat))
)]
fn a_moment_of_the_turn_reads_as_a_condition(
    #[case] text: &str,
    #[case] condition: Option<Condition>,
) {
    assert_eq!(limits(text), (Timing::Instant, None, condition), "{text}");
}

#[rstest]
#[case::once_each_turn(
    "{2}: Draw a card. Activate only during your upkeep and only once each turn.",
    Timing::Instant,
    Some(Limit::OncePerTurn)
)]
#[case::your_turn_once(
    "{2}: Draw a card. Activate only during your turn and only once each turn.",
    Timing::Instant,
    Some(Limit::OncePerTurn)
)]
#[case::once_then_your_turn(
    "{2}: Draw a card. Activate only once each turn and only during your turn.",
    Timing::Instant,
    Some(Limit::OncePerTurn)
)]
#[case::a_sorcery_and_a_moment(
    "{2}: Draw a card. Activate only as a sorcery and only during your turn.",
    Timing::Sorcery,
    None
)]
#[case::a_sorcery_and_a_count_of_a_kind(
    "{2}: Draw a card. Activate only as a sorcery and only if there are at least four creature cards in your graveyard.",
    Timing::Sorcery,
    None
)]
fn parts_join_in_any_order(
    #[case] text: &str,
    #[case] timing: Timing,
    #[case] limit: Option<Limit>,
) {
    let (found_timing, found_limit, condition) = limits(text);
    assert_eq!((found_timing, found_limit), (timing, limit), "{text}");
    assert!(condition.is_some(), "{text}");
}

#[rstest]
#[case::a_count_of_a_kind(
    "{2}{B}, {T}, Sacrifice a creature: Return a Zombie creature card at random from your graveyard to the battlefield. Activate only during your turn and only if there are at least three Zombie creature cards in your graveyard."
)]
#[case::a_state_in_either_order(
    "{2}: Draw a card. Activate only if you control a Swamp and only during your upkeep."
)]
#[case::a_state_after_a_moment_and_a_limit(
    "{2}: Draw a card. Activate only during your upkeep and only if you control a Swamp and only once each turn."
)]
fn a_moment_and_a_state_are_both_held(#[case] text: &str) {
    let (_, _, condition) = limits(text);
    assert!(
        matches!(condition, Some(Condition::All(parts)) if parts.len() >= 2),
        "{text}"
    );
}

#[rstest]
#[case::the_end_step("{2}: Draw a card. Activate only during your end step.")]
#[case::after_attackers(
    "{2}: Draw a card. Activate only during your turn, after attackers are declared."
)]
#[case::combat_damage(
    "{2}: Draw a card. Activate only during combat before combat damage has been dealt."
)]
#[case::another_player("{2}: Draw a card. Activate only during target player's turn.")]
#[case::nothing_after_only("{2}: Draw a card. Activate only")]
#[case::a_moment_without_a_joiner(
    "{2}: Draw a card. Activate only during your turn during combat."
)]
#[case::once_twice("{2}: Draw a card. Activate only once each turn and only once each turn.")]
#[case::a_sorcery_twice("{2}: Draw a card. Activate only as a sorcery and only as a sorcery.")]
#[case::a_state_no_card_reads("{2}: Draw a card. Activate only if a unicorn dances.")]
#[case::fewer_cards(
    "{2}: Draw a card. Activate only if there are at most three Zombie creature cards in your graveyard."
)]
#[case::in_another_graveyard(
    "{2}: Draw a card. Activate only if there are at least three Zombie creature cards in an opponent's graveyard."
)]
fn other_moments_and_conditions_stay_with_the_table(#[case] text: &str) {
    assert!(activated(text).is_none(), "{text}");
}
