//! Goal: the wordings that alone held back two precon cards each (half a life total, rounded up or down) read into the DSL, write back as the same text, and leave their near-misses to the table.
use super::effects::{LATER, dead_card};
use super::*;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;
use crate::trigger::{Step, TriggerPattern};
use crate::value::Amount;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn effects_of(text: &str) -> Vec<Effect> {
    let Ability::Triggered { effects, .. } = golden(text, ClauseKind::Triggered, false) else {
        panic!("not a trigger: {text}");
    };
    effects
}

#[rstest]
#[case::up(
    "Whenever equipped creature deals combat damage to a player, that player loses half their life, rounded up.",
    true
)]
#[case::down(
    "Whenever equipped creature deals combat damage to a player, that player loses half their life, rounded down.",
    false
)]
fn a_player_loses_half_their_life(#[case] text: &str, #[case] up: bool) {
    assert_eq!(
        effects_of(text),
        vec![Effect::LoseLife {
            who: Ref::Triggering,
            amount: Amount::Half {
                of: Box::new(Amount::Life(Ref::Triggering)),
                up
            }
        }],
        "{text}"
    );
}

#[rstest]
#[case::you("When ~ enters, you lose half your life, rounded up.", Ref::You)]
fn the_half_follows_whose_life_it_is(#[case] text: &str, #[case] who: Ref) {
    assert_eq!(
        effects_of(text),
        vec![Effect::LoseLife {
            who: who.clone(),
            amount: Amount::Half {
                of: Box::new(Amount::Life(who)),
                up: true
            }
        }],
        "{text}"
    );
}

#[rstest]
#[case::no_rounding(
    "Whenever ~ deals combat damage to a player, that player loses half their life."
)]
#[case::wrong_rounding(
    "Whenever ~ deals combat damage to a player, that player loses half their life, rounded sideways."
)]
#[case::your_life_for_another_player(
    "Whenever ~ deals combat damage to a player, that player loses half your life, rounded up."
)]
#[case::their_life_for_you("When ~ enters, you lose half their life, rounded up.")]
#[case::each_player("When ~ enters, each player loses half their life, rounded up.")]
#[case::the_cards_of_a_hand(
    "When ~ enters, each opponent discards half the cards in their hand, rounded up."
)]
#[case::gains("Whenever ~ deals combat damage to a player, you gain half your life, rounded up.")]
fn half_a_life_total_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::dies(
    "When ~ dies, return it to its owner's hand at the beginning of the next end step.",
    dead_card()
)]
#[case::dies_by_name(
    "When ~ dies, return ~ to its owner's hand at the beginning of the next end step.",
    dead_card()
)]
#[case::attacks(
    "Whenever ~ attacks, return it to its owner's hand at the beginning of the next end step.",
    Ref::This
)]
fn a_card_returns_to_its_owners_hand_at_the_next_end_step(#[case] text: &str, #[case] card: Ref) {
    let later = Ref::Bound(LATER.to_owned());
    let read = effects_of(text);
    let [
        Effect::Choose { from, bind, .. },
        Effect::DelayedTrigger {
            when,
            once: true,
            effects,
        },
    ] = read.as_slice()
    else {
        panic!("not a choice and a delayed trigger: {text}");
    };
    assert_eq!(*from, card, "{text}");
    assert_eq!(bind, LATER, "{text}");
    assert_eq!(
        *when,
        TriggerPattern::StepBegins {
            step: Step::End,
            whose: Ref::EachPlayer
        },
        "{text}"
    );
    assert_eq!(
        effects,
        &[Effect::Move {
            what: later,
            to: Destination::Hand
        }],
        "{text}"
    );
}

#[rstest]
#[case::your_end_step(
    "When ~ dies, return it to its owner's hand at the beginning of your next end step."
)]
#[case::the_battlefield(
    "When ~ dies, return it to the battlefield at the beginning of the next end step."
)]
#[case::the_graveyard_of_another(
    "Whenever another creature you control dies, return it to its owner's hand at the beginning of the next end step."
)]
#[case::exile("When ~ dies, exile it at the beginning of the next end step.")]
#[case::no_step("When ~ dies, return it to its owner's hand at the beginning of the next.")]
fn a_late_return_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}
