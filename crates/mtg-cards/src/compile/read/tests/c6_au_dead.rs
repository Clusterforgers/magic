//! Goal: the card of another creature that died is found in the graveyard (CR 400.7) when a trigger exiles it, exiles it and does one more thing, or returns it under your control, each written back as the card's text; the card itself and the wordings without a creature that died stay as they were.
use super::*;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

fn the_dead_card() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Graveyard),
        Pred::SameCard(Ref::Triggering),
    ]))
}

fn effects_of(read: Ability) -> Vec<Effect> {
    match read {
        Ability::Triggered { effects, .. } => effects,
        other => panic!("not a trigger: {other:?}"),
    }
}

#[rstest]
#[case::exile_it("Whenever a creature an opponent controls dies, exile it.", 1)]
#[case::and_put_counters(
    "Whenever a creature an opponent controls dies, exile it and put a +1/+1 counter on each Vampire you control.",
    2
)]
#[case::and_you_create(
    "Whenever a nontoken creature enchanted player controls dies, exile it and you create a 1/2 green Spider creature token with reach.",
    2
)]
#[case::another_creature_and_you_create(
    "Whenever another creature you control dies, exile it and you create a 1/1 black Rat creature token.",
    2
)]
fn the_card_that_died_is_exiled_from_the_graveyard(#[case] text: &str, #[case] count: usize) {
    let effects = effects_of(golden(text, ClauseKind::Triggered, false));
    assert_eq!(effects.len(), count, "{text}");
    assert_eq!(
        effects[0],
        Effect::Exile {
            what: the_dead_card(),
            bind: None,
            duration: None,
        },
        "{text}"
    );
}

#[rstest]
#[case::it("When enchanted creature dies, return it to the battlefield under your control.")]
#[case::that_card(
    "When enchanted creature dies, return that card to the battlefield under your control."
)]
#[case::another_creature(
    "Whenever another creature you control dies, return it to the battlefield under your control."
)]
fn the_card_that_died_returns_under_your_control(#[case] text: &str) {
    let effects = effects_of(clause(text, ClauseKind::Triggered, false).expect(text));
    assert_eq!(
        effects,
        vec![Effect::Move {
            what: the_dead_card(),
            to: Destination::Battlefield {
                tapped: false,
                under: Some(Ref::You),
                transformed: false,
                counters: Vec::new(),
            },
        }],
        "{text}"
    );
}

#[rstest]
#[case::enters("Whenever a creature an opponent controls enters, exile it.")]
#[case::attacks("Whenever a creature an opponent controls attacks, exile it and you draw a card.")]
#[case::enters_returns(
    "Whenever another creature you control enters, return that card to the battlefield under your control."
)]
fn without_a_creature_that_died_it_is_not_the_card_in_the_graveyard(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    assert!(
        read.is_none_or(|ability| !format!("{ability:?}").contains("SameCard")),
        "{text}"
    );
}

// dk1b: the card returns to its owner, tapped or not.
#[rstest]
#[case::tapped(
    "When enchanted creature dies, return it to the battlefield tapped under its owner's control.",
    true,
    None
)]
#[case::untapped(
    "When enchanted creature dies, return it to the battlefield under its owner's control.",
    false,
    None
)]
#[case::another_creature(
    "Whenever another creature you control dies, return it to the battlefield under its owner's control.",
    false,
    None
)]
#[case::tapped_under_you(
    "When enchanted creature dies, return it to the battlefield tapped under your control.",
    true,
    Some(Ref::You)
)]
fn dk1b_the_card_that_died_returns_to_its_owner(
    #[case] text: &str,
    #[case] tapped: bool,
    #[case] under: Option<Ref>,
) {
    let effects = effects_of(clause(text, ClauseKind::Triggered, false).expect(text));
    assert_eq!(
        effects,
        vec![Effect::Move {
            what: the_dead_card(),
            to: Destination::Battlefield {
                tapped,
                under,
                transformed: false,
                counters: Vec::new(),
            },
        }],
        "{text}"
    );
}

#[rstest]
#[case::tapped(
    "When enchanted creature dies, return it to the battlefield tapped under its owner's control."
)]
#[case::untapped(
    "When enchanted creature dies, return it to the battlefield under its owner's control."
)]
#[case::tapped_under_you(
    "When enchanted creature dies, return it to the battlefield tapped under your control."
)]
fn dk1b_the_return_to_its_owner_is_written_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::another_player(
    "When enchanted creature dies, return it to the battlefield under an opponent's control."
)]
#[case::tapped_after(
    "When enchanted creature dies, return it to the battlefield under its owner's control tapped."
)]
#[case::enters(
    "Whenever another creature you control enters, return it to the battlefield tapped under its owner's control."
)]
fn dk1b_other_returns_are_left_to_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Triggered, false);
    assert!(
        read.is_none_or(|ability| !format!("{ability:?}").contains("SameCard")),
        "{text}"
    );
}
