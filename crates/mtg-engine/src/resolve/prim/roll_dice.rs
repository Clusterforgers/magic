//! `Effect::RollDice`: a player rolls a die (CR 706) and the resolution goes on with the row of the table the total is in.

use mtg_cards::effect::{DieRow, Effect};

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rng::GameRng;

/// Whether the total is in the range of the row.
fn holds(row: &DieRow, total: i64) -> bool {
    total >= i64::from(row.from) && row.to.is_none_or(|to| total <= i64::from(to))
}

/// The first player the effect names who is still in the game rolls the die: the next substream of the game's random
/// stream gives a number from 1 to the sides, logged as `DieRolled`, and the step goes on into the first row that holds
/// that number plus `plus`, past the effect when no row does.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RollDice {
        who,
        sides,
        plus,
        results,
    } = effect
    else {
        return Err(wrong_effect("RollDice"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let below = GameRng::substream(state.seed(), state.rng_index()).below(u64::from(*sides));
    let result = u32::try_from(below).unwrap_or(0) + 1;
    let total = i64::from(result) + cx.scope.amount(plus)?;
    Ok(Run::Done {
        events: vec![Event::DieRolled {
            player,
            sides: *sides,
            result,
        }],
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: results
            .iter()
            .position(|row| holds(row, total))
            .map_or(Next::Sibling, Next::Enter),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a roll is a number from 1 to the sides from the game's random stream (the same seed gives the same number and the seeds give every face), logged for the player who rolled, and the step enters the first row of the table that holds the number plus the amount added, none when no row does; a player who has left the game rolls nothing, and another effect is refused by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn game(seed: u64) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        game.seed = seed;
        game
    }

    fn roll(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    /// A d6 whose table has three rows: 1 to 2, 3 to 4 and 5 and up.
    fn d6(plus: i32) -> String {
        format!(
            r#"{{"RollDice":{{"sides":6,"plus":{plus},"results":[{{"from":1,"to":2,"effects":[{{"Draw":{{}}}}]}},{{"from":3,"to":4,"effects":[{{"Draw":{{}}}}]}},{{"from":5,"effects":[{{"Draw":{{}}}}]}}]}}}}"#
        )
    }

    /// The number rolled and the row entered by a roll of `json` in the game of `seed`.
    fn rolled(seed: u64, json: &str) -> (u32, Next) {
        let Run::Done { events, next, .. } = roll(&game(seed), json).unwrap() else {
            panic!("a roll asks nothing");
        };
        let [
            Event::DieRolled {
                player,
                sides,
                result,
            },
        ] = events.as_slice()
        else {
            panic!("one roll is logged: {events:?}");
        };
        assert_eq!((*player, *sides), (ME, 6));
        (*result, next)
    }

    #[test]
    fn the_same_seed_rolls_the_same_and_the_seeds_give_every_face() {
        let faces: Vec<u32> = (0..64).map(|seed| rolled(seed, &d6(0)).0).collect();
        assert_eq!(
            faces,
            (0..64)
                .map(|seed| rolled(seed, &d6(0)).0)
                .collect::<Vec<_>>()
        );
        for face in 1..=6 {
            assert!(faces.contains(&face), "no seed rolls a {face}");
        }
        assert!(faces.iter().all(|face| (1..=6).contains(face)));
    }

    #[rstest]
    #[case::no_amount_added(0)]
    #[case::one_added(1)]
    #[case::enough_added_to_leave_the_table(10)]
    #[case::taken_off(-3)]
    fn the_row_is_the_first_that_holds_the_number_with_the_amount_added(#[case] plus: i32) {
        for seed in 0..32 {
            let (result, next) = rolled(seed, &d6(plus));
            let total = i64::from(result) + i64::from(plus);
            let expected = match total {
                ..=0 => Next::Sibling,
                1..=2 => Next::Enter(0),
                3..=4 => Next::Enter(1),
                _ => Next::Enter(2),
            };
            assert_eq!(
                next, expected,
                "seed {seed}: rolled {result}, total {total}"
            );
        }
    }

    #[test]
    fn a_player_who_has_left_rolls_nothing() {
        let mut game = game(1);
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: PlayerId(1),
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let json = d6(0).replace(r#""sides":6"#, r#""who":"Opponents","sides":6"#);
        let Run::Done { events, next, .. } = roll(&game, &json).unwrap() else {
            panic!("a roll asks nothing");
        };
        assert_eq!(events.len(), 0);
        assert_eq!(next, Next::Sibling);
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let error = roll(&game(1), r#"{"Draw":{}}"#).unwrap_err();
        assert!(error.to_string().contains("RollDice"), "{error}");
    }
}
