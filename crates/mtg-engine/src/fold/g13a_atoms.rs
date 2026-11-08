//! Goal: the round 13 facts hold exactly as the game makes them true, at tables of two, three and four seats: a creature attacks alone only while no other creature attacks, it attacks a planeswalker of a player only when that player controls the planeswalker it attacks (attacking the player is another fact), a spell has mana from a kind of source spent on it only for as much as the sources of that kind supplied, a quotient is rounded the way it says and is zero for a divisor of zero, and a difference is never negative.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::spend::{SpendFilter, SpendTraits, SpentFrom};
use mtg_cards::value::Amount;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;
use std::sync::Arc;

use super::apply_event;
use crate::choose::CostTaken;
use crate::combat::AttackTarget;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::turn::Step;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);
const FOE: PlayerId = PlayerId(1);

fn planeswalker(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Planeswalker))
}

/// Seat 0 holds Bear, Wolf and Jace on the battlefield and Bolt in hand; seat 1 holds Liliana; each seat a Guest.
fn table(seats: usize) -> GameState {
    let hand = |seat: usize| {
        if seat == 0 {
            vec![creature("Bolt", 1, 1)]
        } else {
            Vec::new()
        }
    };
    let field = |seat: usize| {
        let mut cards = vec![creature(&format!("Guest {seat}"), 2, 2)];
        if seat == 0 {
            cards.extend([
                creature("Bear", 2, 2),
                creature("Wolf", 2, 2),
                planeswalker("Jace"),
            ]);
        }
        if seat == 1 {
            cards.push(planeswalker("Liliana"));
        }
        cards
    };
    state(
        (0..seats).map(hand).collect(),
        (0..seats).map(field).collect(),
    )
}

fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

fn fold(state: &mut GameState, events: &[Event]) {
    for event in events {
        apply_event(state, event).unwrap();
    }
}

/// Whether the predicate holds for the object, asked by seat `asker`.
fn holds(state: &GameState, asker: PlayerId, pred: Pred, name: &str) -> bool {
    let object = state.object(id(state, name)).unwrap().reference();
    let guest = format!("Guest {}", asker.0);
    let source = state.object(id(state, &guest)).unwrap().reference();
    Scope::announcing(state, asker, source)
        .selects(&Selector(vec![pred]), object)
        .unwrap()
}

/// Seat 0 attacks with the creatures named, each at its target.
fn attacking(seats: usize, attackers: &[(&str, &str)]) -> GameState {
    let mut game = table(seats);
    let attackers = attackers
        .iter()
        .map(|(name, at)| {
            let target = if *at == "foe" {
                AttackTarget::Player(FOE)
            } else {
                AttackTarget::Planeswalker(id(&game, at))
            };
            (id(&game, name), target)
        })
        .collect();
    let events = [
        Event::StepBegan {
            step: Step::BeginCombat,
        },
        Event::AttackersDeclared {
            player: ME,
            attackers,
        },
    ];
    fold(&mut game, &events);
    game
}

#[rstest]
#[case::alone(&[("Bear", "foe")], "Bear", true)]
#[case::with_another(&[("Bear", "foe"), ("Wolf", "foe")], "Bear", false)]
#[case::not_attacking(&[("Bear", "foe")], "Wolf", false)]
fn g13a_a_creature_attacks_alone_while_no_other_does(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[(&str, &str)],
    #[case] name: &str,
    #[case] expected: bool,
) {
    let game = attacking(seats, attackers);
    assert_eq!(holds(&game, FOE, Pred::AttackingAlone, name), expected);
}

#[rstest]
#[case::its_planeswalker("Liliana", FOE, true)]
#[case::another_players_planeswalker("Liliana", ME, false)]
#[case::the_player_not_a_planeswalker("foe", FOE, false)]
fn g13a_a_creature_attacks_a_planeswalker_of_the_player_who_controls_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] at: &str,
    #[case] asker: PlayerId,
    #[case] expected: bool,
) {
    let game = attacking(seats, &[("Bear", at)]);
    let pred = Pred::AttackingPlaneswalkerOf(Ref::You);
    assert_eq!(holds(&game, asker, pred.clone(), "Bear"), expected);
    let either = Pred::AnyOf(vec![Pred::AttackingPlayer(Ref::You), pred]);
    assert!(holds(&game, FOE, either, "Bear"));
}

fn treasure(count: u32) -> SpentFrom {
    SpentFrom {
        traits: SpendTraits {
            subtypes: vec![Subtype("Treasure".into())],
            ..SpendTraits::default()
        },
        count,
    }
}

#[rstest]
#[case::one_from_a_treasure(vec![treasure(1)], 1, true)]
#[case::two_from_treasures_for_three(vec![treasure(1), treasure(1)], 3, false)]
#[case::three_from_treasures(vec![treasure(2), treasure(1)], 3, true)]
#[case::none_from_a_treasure(Vec::new(), 1, false)]
fn g13a_a_spell_has_mana_from_a_kind_of_source_spent_on_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] sources: Vec<SpentFrom>,
    #[case] least: u32,
    #[case] expected: bool,
) {
    let mut game = table(seats);
    let bolt = id(&game, "Bolt");
    let cast = Event::ZoneChanged {
        object: bolt,
        from: Zone::Hand(ME),
        to: Zone::Stack,
        cause: ZoneCause::Moved,
    };
    fold(&mut game, &[cast]);
    game.objects.get_mut(&bolt).unwrap().choices.cost_taken = Some(Box::new(CostTaken {
        sources,
        ..CostTaken::default()
    }));
    let from = SpendFilter {
        subtypes: vec![Subtype("Treasure".into())],
        ..SpendFilter::default()
    };
    let pred = Pred::SpentFrom { from, least };
    assert_eq!(holds(&game, ME, pred.clone(), "Bolt"), expected);
    assert!(!holds(&game, ME, pred, "Bear"));
}

fn quotient(of: i32, by: i32, up: bool) -> Amount {
    Amount::Quotient {
        of: Box::new(Amount::Fixed(of)),
        by: Box::new(Amount::Fixed(by)),
        up,
    }
}

fn difference(left: i32, right: i32) -> Amount {
    Amount::Difference(
        Box::new(Amount::Fixed(left)),
        Box::new(Amount::Fixed(right)),
    )
}

#[rstest]
#[case::a_third_up(quotient(10, 3, true), 4)]
#[case::a_third_down(quotient(10, 3, false), 3)]
#[case::even_up(quotient(9, 3, true), 3)]
#[case::a_tenth_of_one_up(quotient(1, 10, true), 1)]
#[case::by_zero(quotient(7, 0, true), 0)]
#[case::of_less_than_zero(quotient(-4, 3, true), 0)]
#[case::bigger_first(difference(5, 2), 3)]
#[case::smaller_first(difference(2, 5), 3)]
#[case::the_same(difference(4, 4), 0)]
fn g13a_a_quotient_and_a_difference(
    #[values(2, 3, 4)] seats: usize,
    #[case] amount: Amount,
    #[case] expected: i64,
) {
    let game = table(seats);
    let source = game.object(id(&game, "Guest 0")).unwrap().reference();
    let scope = Scope::announcing(&game, ME, source);
    assert_eq!(scope.amount(&amount).unwrap(), expected);
}
