//! Goal: the turn tallies r30-5b adds sum over the players named where their plain twins take the largest: the cards
//! the opponents drew this turn and the noncombat damage they took, at two to four seats, never counting the asking
//! player, and read from the turn that ended last when asked so.
use mtg_cards::value::{Amount, TurnTally};
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::Scope;
use crate::fixtures::{creature, state};
use crate::state::{GameState, Tally};
use crate::zones::Zone;

/// A table of `seats` players, each opponent with the tally `kind` at `each[seat]`; the asking seat 0 has 5 of it.
fn table(seats: usize, kind: Tally, each: &[u32]) -> GameState {
    let battlefield = (0..seats).map(|_| vec![creature("Bear", 2, 2)]).collect();
    let mut game = state((0..seats).map(|_| Vec::new()).collect(), battlefield);
    game.tally_by(PlayerId(0), kind, 5);
    for (seat, count) in each.iter().enumerate().take(seats).skip(1) {
        game.tally_by(PlayerId::from_index(seat), kind, *count);
    }
    game
}

fn amount(game: &GameState, what: TurnTally) -> i64 {
    let source = game
        .object(game.zone(Zone::Battlefield)[0])
        .unwrap()
        .reference();
    let scope = Scope::announcing(game, PlayerId(0), source);
    scope
        .amount(&Amount::Tallied {
            who: mtg_cards::refs::Ref::Opponents,
            what,
        })
        .unwrap()
}

#[rstest]
#[case::two_seats(2, 2, 2)]
#[case::three_seats(3, 5, 3)]
#[case::four_seats(4, 6, 3)]
fn r305b_cards_drawn_add_up_over_the_opponents(
    #[case] seats: usize,
    #[case] total: i64,
    #[case] most: i64,
) {
    let game = table(seats, Tally::Drawn, &[0, 2, 3, 1]);
    assert_eq!(amount(&game, TurnTally::CardsDrawnTotal), total);
    assert_eq!(
        amount(&game, TurnTally::Drawn),
        most,
        "the twin takes the largest"
    );
}

#[rstest]
#[case::two_seats(2, 4)]
#[case::three_seats(3, 4)]
#[case::four_seats(4, 11)]
fn r305b_noncombat_damage_adds_up_over_the_opponents(#[case] seats: usize, #[case] total: i64) {
    let game = table(seats, Tally::E16aNoncombatHurt, &[0, 4, 0, 7]);
    assert_eq!(amount(&game, TurnTally::NoncombatDamageTakenTotal), total);
    let combat = table(seats, Tally::E16aCombatHurt, &[0, 4, 0, 7]);
    assert_eq!(
        amount(&combat, TurnTally::NoncombatDamageTakenTotal),
        0,
        "combat damage is not counted"
    );
}
