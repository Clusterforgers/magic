//! Goal: the first (or nth) time in a turn that a player loses life, cycles a card, surveils or has a token enter under their control, and that counters are put on an object, triggers once for that player or object alone, whoever else did it before; a loss from a payment, from damage and from an effect all count, the events of one batch are counted in order, the count starts again with each turn and the same events folded in one batch or one by one give the same tallies, at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::detect_events::{FOE, ME, fired, fold, id, pattern, target};
use crate::combat::{DamageForm, DamageTarget};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::library::Surveilled;
use crate::state::GameState;
use crate::tokens::TokenCreated;

/// A creature that gains its controller 1 life whenever `when`.
fn watcher(when: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Triggered {
                when: pattern(when),
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(1),
                }],
            }),
    )
}

/// `seats` players: each holds a "Card s", seat 0 controls the Watcher of `when` and a Bear, the others a Guest.
fn table(seats: usize, when: &str) -> GameState {
    let hands = (0..seats)
        .map(|seat| {
            vec![Arc::new(
                CardDefinition::new(&format!("Card {seat}")).with_type(CardType::Instant),
            )]
        })
        .collect();
    let fields = (0..seats)
        .map(|seat| {
            if seat == 0 {
                vec![watcher(when), creature("Bear", 2, 2)]
            } else {
                vec![creature(&format!("Guest {seat}"), 2, 2)]
            }
        })
        .collect();
    state(hands, fields)
}

/// Something that happens to a seat or an object.
#[derive(Debug, Clone, Copy)]
enum Act {
    /// The seat loses 1 life by an effect.
    Lose(usize),
    /// The seat pays 1 life.
    Pay(usize),
    /// The Bear deals 2 damage to the seat.
    Hit(usize),
    /// The seat cycles its card.
    Cycle(usize),
    /// The seat surveils an empty set of cards.
    Surveil(usize),
    /// A token enters under the seat's control.
    Token(usize),
    /// A +1/+1 counter is put on the named permanent.
    Counter(&'static str),
}
use Act::{Counter, Cycle, Hit, Lose, Pay, Surveil, Token};

fn player(seat: usize) -> PlayerId {
    PlayerId(u8::try_from(seat).unwrap().into())
}

fn events(pre: &GameState, acts: &[Act]) -> Vec<Event> {
    let next = pre.next_object().0;
    acts.iter()
        .enumerate()
        .map(|(n, act)| match *act {
            Lose(seat) => Event::LifeLost {
                player: player(seat),
                amount: 1,
            },
            Pay(seat) => Event::LifePaid {
                player: player(seat),
                amount: 1,
            },
            Hit(seat) => Event::DamageDealt {
                source: id(pre, "Bear"),
                target: DamageTarget::Player(player(seat)),
                amount: 2,
                deathtouch: false,
                form: DamageForm::Normal,
                toxic: 0,
            },
            Cycle(seat) => Event::Cycled {
                player: player(seat),
                object: id(pre, &format!("Card {seat}")),
            },
            Surveil(seat) => Event::Surveilled(Surveilled {
                player: player(seat),
                top: Vec::new(),
                graveyard: Vec::new(),
            }),
            Token(seat) => Event::TokenCreated(TokenCreated {
                object: ObjectId(next + u64::try_from(n).unwrap()),
                copy_of: None,
                controller: player(seat),
                name: "Treasure".into(),
                tapped: false,
                attacking: None,
                definition: Arc::new(CardDefinition::new("Treasure").with_type(CardType::Artifact)),
            }),
            Counter(name) => {
                let on = target(pre, name);
                Event::CountersChanged(CountersChanged {
                    on,
                    counter: CounterKind::PlusOnePlusOne,
                    delta: 1,
                })
            }
        })
        .collect()
}

/// How many abilities the last of `acts` triggers, once the ones before it have happened.
fn last_fires(pre: &GameState, acts: &[Act]) -> usize {
    let all = events(pre, acts);
    let (before, last) = all.split_at(all.len() - 1);
    fired(&fold(pre, before), last)
}

const LOSS_1: &str = r#"{"Nth":{"event":{"LifeLost":{"who":"You"}},"nth":1}}"#;
const LOSS_2: &str = r#"{"Nth":{"event":{"LifeLost":{"who":"You"}},"nth":2}}"#;
const THEIR_LOSS_1: &str = r#"{"Nth":{"event":{"LifeLost":{"who":"Opponents"}},"nth":1}}"#;
const CYCLE_1: &str = r#"{"Nth":{"event":{"Cycled":{"what":[],"by":"You"}},"nth":1}}"#;
const CYCLE_2: &str = r#"{"Nth":{"event":{"Cycled":{"what":[],"by":"You"}},"nth":2}}"#;
const SURVEIL_1: &str = r#"{"Nth":{"event":{"Surveilled":{"who":"You"}},"nth":1}}"#;
const SURVEIL_2: &str = r#"{"Nth":{"event":{"Surveilled":{"who":"You"}},"nth":2}}"#;
const TOKEN_1: &str = r#"{"Nth":{"event":{"ZoneChange":{"what":["Token",{"ControlledBy":"You"}],"to":"Battlefield"}},"nth":1}}"#;
const TOKEN_2: &str = r#"{"Nth":{"event":{"ZoneChange":{"what":["Token",{"ControlledBy":"You"}],"to":"Battlefield"}},"nth":2}}"#;
const COUNTER_1: &str = r#"{"Nth":{"event":{"CountersPut":{"what":["This"]}},"nth":1}}"#;
const COUNTER_2: &str = r#"{"Nth":{"event":{"CountersPut":{"what":["This"]}},"nth":2}}"#;

#[rstest]
#[case::the_first_loss(LOSS_1, &[], Lose(0), 1)]
#[case::not_the_second_loss(LOSS_1, &[Lose(0)], Lose(0), 0)]
#[case::a_payment_is_a_loss(LOSS_1, &[], Pay(0), 1)]
#[case::damage_is_a_loss(LOSS_1, &[], Hit(0), 1)]
#[case::a_payment_then_damage(LOSS_1, &[Pay(0)], Hit(0), 0)]
#[case::damage_then_an_effect(LOSS_1, &[Hit(0)], Lose(0), 0)]
#[case::the_second_loss(LOSS_2, &[Lose(0)], Pay(0), 1)]
#[case::not_the_first_of_two(LOSS_2, &[], Lose(0), 0)]
#[case::not_the_third(LOSS_2, &[Lose(0), Lose(0)], Lose(0), 0)]
#[case::an_opponents_loss_is_not_mine(LOSS_1, &[], Lose(1), 0)]
#[case::an_opponents_loss_does_not_use_mine_up(LOSS_1, &[Lose(1)], Lose(0), 1)]
#[case::the_first_loss_of_an_opponent(THEIR_LOSS_1, &[], Lose(1), 1)]
#[case::not_my_loss_for_an_opponent_pattern(THEIR_LOSS_1, &[], Lose(0), 0)]
#[case::each_opponent_has_their_own_first(THEIR_LOSS_1, &[Lose(1)], Hit(1), 0)]
#[case::the_first_cycling(CYCLE_1, &[], Cycle(0), 1)]
#[case::not_the_second_cycling(CYCLE_1, &[Cycle(0)], Cycle(0), 0)]
#[case::the_second_cycling(CYCLE_2, &[Cycle(0)], Cycle(0), 1)]
#[case::an_opponents_cycling_is_not_mine(CYCLE_1, &[], Cycle(1), 0)]
#[case::an_opponents_cycling_does_not_use_mine_up(CYCLE_1, &[Cycle(1)], Cycle(0), 1)]
#[case::a_loss_is_not_a_cycling(CYCLE_1, &[Lose(0)], Cycle(0), 1)]
#[case::the_first_surveil(SURVEIL_1, &[], Surveil(0), 1)]
#[case::not_the_second_surveil(SURVEIL_1, &[Surveil(0)], Surveil(0), 0)]
#[case::the_second_surveil(SURVEIL_2, &[Surveil(0)], Surveil(0), 1)]
#[case::an_opponents_surveil_is_not_mine(SURVEIL_1, &[], Surveil(1), 0)]
#[case::an_opponents_surveil_does_not_use_mine_up(SURVEIL_1, &[Surveil(1)], Surveil(0), 1)]
#[case::the_first_token(TOKEN_1, &[], Token(0), 1)]
#[case::not_the_second_token(TOKEN_1, &[Token(0)], Token(0), 0)]
#[case::the_second_token(TOKEN_2, &[Token(0)], Token(0), 1)]
#[case::an_opponents_token_is_not_mine(TOKEN_1, &[], Token(1), 0)]
#[case::an_opponents_token_does_not_use_mine_up(TOKEN_1, &[Token(1)], Token(0), 1)]
#[case::the_first_counter_on_it(COUNTER_1, &[], Counter("Watcher"), 1)]
#[case::not_the_second_counter_on_it(COUNTER_1, &[Counter("Watcher")], Counter("Watcher"), 0)]
#[case::the_second_counter_on_it(COUNTER_2, &[Counter("Watcher")], Counter("Watcher"), 1)]
#[case::a_counter_on_another_is_not_on_it(COUNTER_1, &[], Counter("Bear"), 0)]
#[case::a_counter_on_another_does_not_use_it_up(COUNTER_1, &[Counter("Bear")], Counter("Watcher"), 1)]
fn the_nth_time_of_the_turn_counts_for_the_player_or_object_alone(
    #[case] when: &str,
    #[case] before: &[Act],
    #[case] last: Act,
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut acts = before.to_vec();
    acts.push(last);
    assert_eq!(last_fires(&table(seats, when), &acts), expected);
}

#[rstest]
#[case::two_losses_in_one_batch(LOSS_1, &[Lose(0), Pay(0)], 1)]
#[case::the_second_of_a_batch(LOSS_2, &[Lose(0), Pay(0)], 1)]
#[case::damage_and_a_loss_in_one_batch(LOSS_2, &[Hit(0), Lose(0), Pay(0)], 1)]
#[case::two_cyclings_in_one_batch(CYCLE_1, &[Cycle(0), Cycle(0)], 1)]
#[case::two_surveils_in_one_batch(SURVEIL_1, &[Surveil(0), Surveil(0)], 1)]
#[case::two_tokens_in_one_batch(TOKEN_1, &[Token(0), Token(0), Token(0)], 1)]
#[case::the_second_token_of_a_batch(TOKEN_2, &[Token(0), Token(0), Token(0)], 1)]
#[case::two_counters_in_one_batch(COUNTER_1, &[Counter("Watcher"), Counter("Watcher")], 1)]
#[case::an_opponents_first_is_theirs(THEIR_LOSS_1, &[Lose(1), Lose(0), Lose(1)], 1)]
fn the_events_of_one_batch_are_counted_in_order(
    #[case] when: &str,
    #[case] acts: &[Act],
    #[case] expected: usize,
) {
    let pre = table(2, when);
    assert_eq!(fired(&pre, &events(&pre, acts)), expected);
}

#[rstest]
#[case::loss(LOSS_1, Lose(0))]
#[case::cycling(CYCLE_1, Cycle(0))]
#[case::surveil(SURVEIL_1, Surveil(0))]
#[case::token(TOKEN_1, Token(0))]
#[case::counter(COUNTER_1, Counter("Watcher"))]
fn a_new_turn_starts_the_count_again(#[case] when: &str, #[case] act: Act) {
    let pre = table(2, when);
    let first = fold(&pre, &events(&pre, &[act]));
    assert!(
        !first.tallies.players.is_empty() || !first.tallies.objects.is_empty(),
        "the event is counted"
    );
    assert_eq!(fired(&first, &events(&first, &[act])), 0, "the second time");
    let turn = Event::TurnBegan {
        number: 2,
        active: FOE,
    };
    let next = fold(&first, &[turn]);
    assert!(next.tallies.players.is_empty() && next.tallies.objects.is_empty());
    assert_eq!(
        fired(&next, &events(&next, &[act])),
        1,
        "the first of the new turn"
    );
}

#[rstest]
#[case::losses(LOSS_1, &[Lose(0), Hit(1), Pay(0), Hit(0)])]
#[case::cyclings(CYCLE_1, &[Cycle(0), Cycle(1), Cycle(0)])]
#[case::surveils(SURVEIL_1, &[Surveil(0), Surveil(1), Surveil(0)])]
#[case::tokens(TOKEN_1, &[Token(0), Token(1), Token(0)])]
#[case::counters(COUNTER_1, &[Counter("Watcher"), Counter("Bear"), Counter("Watcher")])]
fn a_replay_of_the_log_gives_the_same_tallies_whatever_the_batches(
    #[case] when: &str,
    #[case] acts: &[Act],
) {
    let pre = table(3, when);
    let all = events(&pre, acts);
    let whole = fold(&pre, &all);
    let mut one_by_one = pre.clone();
    for event in &all {
        one_by_one = fold(&one_by_one, std::slice::from_ref(event));
    }
    assert_eq!(whole.tallies.players, one_by_one.tallies.players);
    assert_eq!(whole.tallies.objects, one_by_one.tallies.objects);
    assert_eq!(whole.state_hash(), one_by_one.state_hash());
}

#[test]
fn damage_that_causes_no_life_loss_is_not_a_loss() {
    let pre = table(2, LOSS_1);
    let bear = id(&pre, "Bear");
    let hit = |form| Event::DamageDealt {
        source: bear,
        target: DamageTarget::Player(ME),
        amount: 2,
        deathtouch: false,
        form,
        toxic: 0,
    };
    assert_eq!(fired(&pre, &[hit(DamageForm::Infect)]), 0);
    assert_eq!(fired(&pre, &[hit(DamageForm::NoLifeLoss)]), 0);
    assert_eq!(fired(&pre, &[hit(DamageForm::Normal)]), 1);
}
