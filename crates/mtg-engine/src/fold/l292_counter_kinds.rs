//! Goal: `Amount::Distinct` over `StatKind::CounterKinds` counts each kind of counter once among the objects the selector names ("for each kind of counter among permanents you control"), a kind on several objects once and an object without counters not at all, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, CounterKind, StatKind};
use mtg_core::ids::PlayerId;
use rstest::rstest;

use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// Seat 0 controls three creatures: one with a +1/+1 and a charge counter, one with a +1/+1 counter, one with none;
/// every other seat one creature with a -1/-1 counter.
fn table(seats: usize) -> GameState {
    let field = |seat: usize| {
        if seat == 0 {
            vec![
                creature("A", 2, 2),
                creature("B", 2, 2),
                creature("C", 2, 2),
            ]
        } else {
            vec![creature("D", 2, 2)]
        }
    };
    let mut game = state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats).map(field).collect(),
    );
    let put = |game: &mut GameState, name: &str, counter: CounterKind| {
        let id = game
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| game.object(*id).unwrap().card().name == name)
            .unwrap();
        let on = Target::Object(game.object(id).unwrap().reference());
        crate::fold::apply_event(
            game,
            &Event::CountersChanged(CountersChanged {
                on,
                counter,
                delta: 1,
            }),
        )
        .unwrap();
    };
    put(&mut game, "A", CounterKind::PlusOnePlusOne);
    put(&mut game, "A", CounterKind::Other("charge".to_owned()));
    put(&mut game, "B", CounterKind::PlusOnePlusOne);
    put(&mut game, "D", CounterKind::MinusOneMinusOne);
    game
}

fn kinds(state: &GameState, of: Vec<Pred>) -> i64 {
    let source = state.zone(Zone::Battlefield)[0];
    let source = state.object(source).unwrap().reference();
    Scope::announcing(state, PlayerId(0), source)
        .amount(&Amount::Distinct {
            of: Selector(of),
            stat: StatKind::CounterKinds,
        })
        .unwrap()
}

#[rstest]
#[case::mine(vec![Pred::ControlledBy(Ref::You)], 2)]
#[case::theirs(vec![Pred::ControlledBy(Ref::Opponents)], 1)]
#[case::everything(vec![], 3)]
#[case::named_without_counters(vec![Pred::Named("C".to_owned())], 0)]
fn each_kind_of_counter_counts_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] of: Vec<Pred>,
    #[case] expected: i64,
) {
    assert_eq!(kinds(&table(seats), of), expected);
}
