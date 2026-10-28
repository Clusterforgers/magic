//! Goal: the wordings that alone held back two precon cards each (half a life total, rounded up or down) read into the DSL, write back as the same text, and leave their near-misses to the table.
use super::effects::{EXILED_CARD, LATER, dead_card};
use super::*;
use crate::compile::read::draws;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
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
            this_turn: false,
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
#[case::exile("When ~ dies, exile it at the beginning of the next end step.")]
#[case::no_step("When ~ dies, return it to its owner's hand at the beginning of the next.")]
fn a_late_return_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::a_spell(
    "Each player discards their hand, then draws cards equal to the greatest number of cards a player discarded this way.",
    ClauseKind::Spell
)]
#[case::an_ability(
    "{U}, {T}: Each player discards their hand, then draws cards equal to the greatest number of cards a player discarded this way.",
    ClauseKind::Activated
)]
fn each_player_draws_the_most_cards_a_player_discarded(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    let read = golden(text, kind, false);
    let effects = match read {
        Ability::Spell { effects, .. } | Ability::Activated { effects, .. } => effects,
        other => panic!("not a spell or an ability: {other:?}"),
    };
    assert_eq!(
        effects,
        vec![
            Effect::Discard {
                who: Ref::EachPlayer,
                count: draws::hand_of(&Ref::EachPlayer),
                random: false,
                bind: Some(draws::DISCARDED.to_owned()),
            },
            Effect::Draw {
                who: Ref::EachPlayer,
                count: draws::most_discarded(),
            },
        ],
        "{text}"
    );
}

#[rstest]
#[case::the_wheel("Each player discards their hand, then draws seven cards.", None)]
fn a_plain_wheel_binds_nothing(#[case] text: &str, #[case] bind: Option<String>) {
    let Ability::Spell { effects, .. } = golden(text, ClauseKind::Spell, false) else {
        panic!("not a spell: {text}");
    };
    assert!(
        matches!(&effects[0], Effect::Discard { bind: found, .. } if *found == bind),
        "{text}"
    );
}

#[rstest]
#[case::a_player("{T}: Target player exiles a card from their graveyard.", "Player")]
#[case::an_opponent("{T}: Target opponent exiles a card from their graveyard.", "Opponent")]
#[case::with_a_cost(
    "{2}, {T}: Target player exiles a card from their graveyard.",
    "Player"
)]
fn a_player_exiles_a_card_of_their_own_graveyard(#[case] text: &str, #[case] target: &str) {
    let Ability::Activated {
        targets, effects, ..
    } = golden(text, ClauseKind::Activated, false)
    else {
        panic!("not an activated ability: {text}");
    };
    assert_eq!(
        serde_json::to_string(&targets[0].what).unwrap(),
        format!("\"{target}\""),
        "{text}"
    );
    assert_eq!(
        effects,
        vec![
            Effect::Choose {
                who: Ref::Target(0),
                from: Ref::Each(Selector(vec![
                    Pred::In(ZoneKind::Graveyard),
                    Pred::OwnedBy(Ref::Target(0)),
                ])),
                count: Bounds::default(),
                bind: EXILED_CARD.to_owned(),
                rest_as: None,
            },
            Effect::Exile {
                what: Ref::Bound(EXILED_CARD.to_owned()),
                bind: None,
                duration: None,
            },
        ],
        "{text}"
    );
}

#[rstest]
#[case::your_own("{T}: Exile a card from your graveyard.")]
#[case::a_hand("{T}: Target player exiles a card from their hand.")]
#[case::two_cards("{T}: Target player exiles two cards from their graveyard.")]
#[case::any_graveyard("{T}: Target player exiles a card from a graveyard.")]
#[case::your_graveyard("{T}: Target player exiles a card from your graveyard.")]
#[case::each_player("{T}: Each player exiles a card from their graveyard.")]
#[case::you("{T}: You exile a card from their graveyard.")]
fn exiling_a_card_from_a_graveyard_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Activated, false), None, "{text}");
}

#[rstest]
#[case::the_number_of_cards(
    "Each player discards their hand, then draws cards equal to the number of cards a player discarded this way."
)]
#[case::the_least(
    "Each player discards their hand, then draws cards equal to the least number of cards a player discarded this way."
)]
#[case::exiled_this_way(
    "Each player discards their hand, then draws cards equal to the greatest number of cards a player exiled this way."
)]
#[case::only_you(
    "You discard your hand, then draw cards equal to the greatest number of cards a player discarded this way."
)]
#[case::one_discard_for_all(
    "Each player discards a card, then draws cards equal to the greatest number of cards a player discarded this way."
)]
fn the_most_discarded_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
