//! Goal: the triggers that count an event of the turn ("for the first time each turn" after gaining or losing life, cycling, surveilling, creating tokens and having counters put on the card), the zone change "from anywhere other than the battlefield" and the condition "a counter was put on ~ this turn" read into the patterns the engine runs and write back as the card's own text, and the near-misses stay unread.
use super::*;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, TurnTally};

#[rstest]
#[case::gain_life(
    "Whenever you gain life for the first time each turn, surveil 1.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"LifeGained":{"who":"You"}},"nth":1}},"effects":[{"Surveil":{}}]}}"#
)]
#[case::lose_life(
    "Whenever you lose life for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"LifeLost":{"who":"You"}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::an_opponent_loses_life(
    "Whenever an opponent loses life for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"LifeLost":{"who":"Opponents"}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::cycle_another_card(
    "Whenever you cycle another card for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Cycled":{"what":["Other"],"by":"You"}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::cycle_a_card_the_second_time(
    "Whenever you cycle a card for the second time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Cycled":{"what":[],"by":"You"}},"nth":2}},"effects":[{"Draw":{}}]}}"#
)]
#[case::surveil(
    "Whenever you surveil for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"Surveilled":{"who":"You"}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::create_tokens(
    "Whenever you create one or more tokens for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"ZoneChange":{"what":["Token",{"ControlledBy":"You"}],"to":"Battlefield"}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
#[case::counters_put_on_the_card(
    "Whenever one or more counters are put on ~ for the first time each turn, draw a card.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"CountersPut":{"what":["This"]}},"nth":1}},"effects":[{"Draw":{}}]}}"#
)]
fn an_event_of_the_turn_counted_reads_and_writes_back(#[case] text: &str, #[case] written: &str) {
    reads_as(text, ClauseKind::Triggered, false, written);
}

#[rstest]
#[case::a_creature(
    "Whenever a creature is put into a graveyard from anywhere other than the battlefield, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChangeExcept":{"what":[{"Type":"Creature"}],"except":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::into_your_graveyard(
    "Whenever a creature is put into your graveyard from anywhere other than the battlefield, draw a card.",
    r#"{"Triggered":{"when":{"ZoneChangeExcept":{"what":[{"Type":"Creature"},{"OwnedBy":"You"}],"except":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{}}]}}"#
)]
fn a_card_put_into_a_graveyard_from_anywhere_but_the_battlefield_reads_and_writes_back(
    #[case] text: &str,
    #[case] written: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, written);
}

#[rstest]
#[case::a_counter_put_on_it(
    "At the beginning of your end step, if a counter was put on ~ this turn, draw a card."
)]
fn the_condition_on_counters_put_this_turn_reads(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    let Ability::Triggered {
        intervening_if: Some(condition),
        ..
    } = read
    else {
        panic!("{text}: no condition");
    };
    assert_eq!(
        condition,
        Condition::Compare {
            left: Amount::Tallied {
                who: Ref::This,
                what: TurnTally::CountersPut,
            },
            op: Cmp::Ge,
            right: Amount::Fixed(1),
        }
    );
}

#[rstest]
#[case::the_zeroth_time("Whenever you surveil for the zeroth time each turn, draw a card.")]
#[case::once_per_game("Whenever you surveil for the first time, draw a card.")]
#[case::one_token("Whenever you create a token for the first time each turn, draw a card.")]
#[case::an_opponents_tokens(
    "Whenever an opponent creates one or more tokens for the first time each turn, draw a card."
)]
#[case::a_count_of_cycling_itself("When you cycle ~ for the first time each turn, draw a card.")]
#[case::a_kind_of_counter(
    "Whenever one or more +1/+1 counters are put on ~ for the first time each turn, draw a card."
)]
#[case::from_a_named_zone(
    "Whenever a creature card is put into a graveyard from anywhere other than your hand, draw a card."
)]
fn other_wordings_of_the_counted_events_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
