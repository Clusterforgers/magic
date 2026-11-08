//! Goal: `Amount::Distinct` over `StatKind::Name` counts each card name once among the objects the selector names, whoever controls them, and a name is no number for a comparison, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::{Amount, StatKind};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;

/// Seat 0 controls two Bears and an Elk; every other seat one Bear.
fn table(seats: usize) -> GameState {
    let field = |seat: usize| {
        if seat == 0 {
            vec![
                creature("Bear", 2, 2),
                creature("Bear", 2, 2),
                creature("Elk", 3, 3),
            ]
        } else {
            vec![creature("Bear", 2, 2)]
        }
    };
    state(
        (0..seats).map(|_| Vec::new()).collect(),
        (0..seats).map(field).collect(),
    )
}

fn distinct(state: &GameState, of: Vec<Pred>) -> i64 {
    let source = state.zone(crate::zones::Zone::Battlefield)[0];
    let source = state.object(source).unwrap().reference();
    Scope::announcing(state, PlayerId(0), source)
        .amount(&Amount::Distinct {
            of: Selector(of),
            stat: StatKind::Name,
        })
        .unwrap()
}

#[rstest]
#[case::everything(vec![], 2)]
#[case::creatures(vec![Pred::Type(CardType::Creature)], 2)]
#[case::mine(vec![Pred::ControlledBy(Ref::You)], 2)]
#[case::theirs(vec![Pred::ControlledBy(Ref::Opponents)], 1)]
#[case::lands(vec![Pred::Type(CardType::Land)], 0)]
fn each_name_counts_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] of: Vec<Pred>,
    #[case] expected: i64,
) {
    assert_eq!(distinct(&table(seats), of), expected);
}
