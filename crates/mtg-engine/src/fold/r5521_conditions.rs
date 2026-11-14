//! Goal: "the Ring has tempted you N or more times this game" reads each player's temptations as the `RingTempted`
//! events left them, and "you've completed <dungeon>" reads the dungeons each player completed by name, at two to four
//! seats, for the reader and for its opponents.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::refs::Ref;
use mtg_cards::value::Condition;
use mtg_cards::w11d_dungeon::w11d_dungeons;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::apply_event;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

const ME: PlayerId = PlayerId(0);

/// Each seat controls a Bear.
fn r5521_table(seats: usize) -> GameState {
    state(
        vec![Vec::new(); seats],
        (0..seats).map(|_| vec![creature("Bear", 2, 2)]).collect(),
    )
}

fn r5521_holds(game: &GameState, condition: &Condition) -> bool {
    let source = game
        .object(game.zone(Zone::Battlefield)[0])
        .unwrap()
        .reference();
    Scope::announcing(game, ME, source)
        .holds(condition)
        .unwrap()
}

/// Folds `times` temptations of the player, the level rising to 4 at most.
fn r5521_tempt(game: &mut GameState, player: PlayerId, times: u8) {
    for _ in 0..times {
        let level = (game.ring_level(player) + 1).min(4);
        apply_event(
            game,
            &Event::RingTempted {
                player,
                level,
                bearer: None,
            },
        )
        .unwrap();
    }
}

#[rstest]
fn r5521_temptations_are_counted_per_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 5)] mine: u8,
    #[values(0, 3)] theirs: u8,
    #[values(2, 4)] times: u32,
) {
    let mut game = r5521_table(seats);
    r5521_tempt(&mut game, ME, mine);
    r5521_tempt(&mut game, PlayerId::from_index(seats - 1), theirs);
    let you = Condition::RingTempted {
        who: Ref::You,
        times,
    };
    let foes = Condition::RingTempted {
        who: Ref::Opponents,
        times,
    };
    assert_eq!(r5521_holds(&game, &you), u32::from(mine) >= times);
    assert_eq!(r5521_holds(&game, &foes), u32::from(theirs) >= times);
}

#[rstest]
fn r5521_completed_dungeons_are_read_by_name(
    #[values(2, 3, 4)] seats: usize,
    #[values(None, Some("Tomb of Annihilation"), Some("Lost Mine of Phandelver"))] mine: Option<
        &str,
    >,
    #[values(None, Some("Tomb of Annihilation"))] theirs: Option<&str>,
) {
    let mut game = r5521_table(seats);
    let index = |name: &str| {
        u8::try_from(
            w11d_dungeons()
                .unwrap()
                .iter()
                .position(|d| d.name == name)
                .unwrap(),
        )
        .unwrap()
    };
    let foe = PlayerId::from_index(seats - 1);
    for (player, done) in [(ME, mine), (foe, theirs)] {
        if let Some(name) = done {
            game.w11d
                .completed_named
                .entry(player)
                .or_default()
                .insert(index(name));
        }
    }
    let tomb = |who| Condition::CompletedNamedDungeon {
        who,
        dungeon: "Tomb of Annihilation".into(),
    };
    assert_eq!(
        r5521_holds(&game, &tomb(Ref::You)),
        mine == Some("Tomb of Annihilation")
    );
    assert_eq!(r5521_holds(&game, &tomb(Ref::Opponents)), theirs.is_some());
    assert_eq!(
        game.r5521_completed_named(ME, "Lost Mine of Phandelver"),
        mine == Some("Lost Mine of Phandelver")
    );
}

#[rstest]
#[case::alone_fastest(&[1, 3, 2, 2])]
#[case::tied(&[1, 3, 3, 2])]
#[case::slower(&[4, 3, 2, 2])]
#[case::no_speed(&[0, 0, 0, 0])]
fn r5521_the_fastest_player_is_strictly_ahead_of_each_other(
    #[values(2, 3, 4)] seats: usize,
    #[case] speeds: &[u8],
) {
    let mut game = r5521_table(seats);
    for (seat, speed) in speeds.iter().take(seats).enumerate() {
        let player = PlayerId::from_index(seat);
        apply_event(
            &mut game,
            &Event::SpeedChanged {
                player,
                speed: *speed,
            },
        )
        .unwrap();
    }
    let ahead = |at: usize| {
        (0..seats)
            .filter(|seat| *seat != at)
            .all(|seat| speeds[seat] < speeds[at])
    };
    // "You" is seat 0; with one opponent "an opponent" is seat 1, and two or more opponents are never all ahead.
    assert_eq!(
        r5521_holds(&game, &Condition::FastestPlayer(Ref::You)),
        ahead(0)
    );
    assert_eq!(
        r5521_holds(&game, &Condition::FastestPlayer(Ref::Opponents)),
        seats == 2 && ahead(1)
    );
}
