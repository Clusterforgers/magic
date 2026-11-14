//! `Effect::ReduceSpeed`: players' speeds go down by 1, never below 1 (CR 702.179).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each player the effect names who is in the game with a speed above 1 gets one `SpeedChanged` a step lower; a speed of
/// 0 or 1 is left as it is.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ReduceSpeed { who } = effect else {
        return Err(wrong_effect("ReduceSpeed"));
    };
    let state = cx.state();
    let mut players = cx.scope.players(who)?;
    players.dedup();
    Ok(Run::events(
        players
            .into_iter()
            .filter(|player| state.is_alive(*player))
            .filter_map(|player| {
                let speed = state.speed(player);
                (speed > 1).then(|| Event::SpeedChanged {
                    player,
                    speed: speed - 1,
                })
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: a reduction lowers each named player's speed by one and never below 1, leaves the others alone at two to
    //! four seats, and refuses another effect.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn r5521_game(seats: usize, speeds: &[u8]) -> GameState {
        let mut game = state(
            vec![Vec::new(); seats],
            (0..seats).map(|_| vec![creature("Bear", 2, 2)]).collect(),
        );
        for (seat, speed) in speeds.iter().enumerate() {
            apply_event(
                &mut game,
                &Event::SpeedChanged {
                    player: PlayerId::from_index(seat),
                    speed: *speed,
                },
            )
            .unwrap();
        }
        game
    }

    fn r5521_run(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    #[rstest]
    fn r5521_opponents_slow_down_but_not_below_one(
        #[values(2, 3, 4)] seats: usize,
        #[values(0, 1, 2, 4)] theirs: u8,
    ) {
        let mut speeds = vec![3; seats];
        speeds[1..].fill(theirs);
        let mut game = r5521_game(seats, &speeds);
        let Run::Done { events, .. } =
            r5521_run(&game, r#"{"ReduceSpeed":{"who":"Opponents"}}"#).unwrap()
        else {
            panic!("events");
        };
        for event in &events {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(game.speed(ME), 3);
        let expected = if theirs > 1 { theirs - 1 } else { theirs };
        for seat in 1..seats {
            assert_eq!(game.speed(PlayerId::from_index(seat)), expected);
        }
    }

    #[test]
    fn r5521_another_effect_is_refused() {
        let game = r5521_game(2, &[]);
        assert!(r5521_run(&game, r#"{"Draw":{}}"#).is_err());
    }
}
