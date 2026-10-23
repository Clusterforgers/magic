//! `Effect::Mill`: players mill cards (CR 701.17).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each player the effect names who is still in the game puts the top cards of their library into their graveyard,
/// the top card first and all in one step, in turn order from the player the reference lists first. A library shorter
/// than the count gives up what it has, an empty one gives nothing and a count of nothing is no event; milling is not
/// drawing, so no player loses by it. Every move is a `ZoneChanged` with `ZoneCause::Milled`, and a graveyard is open,
/// so every seat sees which cards were milled.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Mill { who, count } = effect else {
        return Err(wrong_effect("Mill"));
    };
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        events.extend(top_cards(state, player, wanted).into_iter().map(|object| {
            Event::ZoneChanged {
                object,
                from: Zone::Library(player),
                to: Zone::Graveyard(player),
                cause: ZoneCause::Milled,
            }
        }));
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: a mill moves the top cards of each library the effect names to its owner's graveyard, the top card first and all in one step, for any count the value evaluator reads, in turn order over the players it names; a library shorter than the count gives what it has, an empty library, a count of nothing and a player who has left give nothing; nothing else changes (no draw, no loss), it binds nothing, goes on to the next effect and refuses another effect or a count it cannot read by name.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::player::LossReason;
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    /// `seats` seats, each with a library of `Library 0` (on top) to `Library 4`; seat 0 controls the Source and a Wolf.
    /// The libraries of the seats in `empty` hold nothing and the seats in `out` have left the game.
    fn game(seats: usize, empty: &[usize], out: &[usize]) -> GameState {
        let mut battlefield = vec![vec![creature("Source", 1, 1), creature("Wolf", 2, 2)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        let mut setup = setup(vec![Vec::new(); seats], battlefield);
        for &seat in empty {
            setup.players[seat].library.clear();
        }
        let mut game = GameState::from_setup(&setup).unwrap();
        for &seat in out {
            apply_event(
                &mut game,
                &Event::PlayerLost {
                    player: PlayerId::from_index(seat),
                    reason: LossReason::Conceded,
                },
            )
            .unwrap();
        }
        game
    }

    fn run_on(game: &GameState, json: &str) -> Result<Run, ResolveError> {
        let effect: Effect = serde_json::from_str(json).unwrap();
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &effect)
    }

    /// "seat:card" for each card milled, in the order of the events.
    fn milled(game: &GameState, json: &str) -> Vec<String> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run_on(game, json).unwrap()
        else {
            panic!("a mill needs no answer");
        };
        assert!(bound.is_empty() && snapshots.is_empty());
        assert_eq!(next, Next::Sibling);
        events
            .iter()
            .map(|event| match event {
                Event::ZoneChanged {
                    object,
                    from: Zone::Library(from),
                    to: Zone::Graveyard(to),
                    cause: ZoneCause::Milled,
                } => {
                    assert_eq!(from, to, "a card goes to the graveyard of its owner");
                    let card = game.object(*object).unwrap();
                    assert_eq!(card.owner(), *from);
                    format!("{}:{}", from.index(), card.card().name)
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[rstest]
    #[case::one_card_by_default(2, r#"{"Mill":{}}"#, &["0:Library 0"])]
    #[case::three_cards_top_first(2, r#"{"Mill":{"count":3}}"#, &["0:Library 0", "0:Library 1", "0:Library 2"])]
    #[case::the_whole_library(2, r#"{"Mill":{"count":5}}"#, &["0:Library 0", "0:Library 1", "0:Library 2", "0:Library 3", "0:Library 4"])]
    #[case::more_than_the_library_has(2, r#"{"Mill":{"count":9}}"#, &["0:Library 0", "0:Library 1", "0:Library 2", "0:Library 3", "0:Library 4"])]
    #[case::a_count_of_nothing(2, r#"{"Mill":{"count":0}}"#, &[])]
    #[case::a_count_below_nothing(2, r#"{"Mill":{"count":-2}}"#, &[])]
    #[case::a_count_the_board_decides(
        2,
        r#"{"Mill":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#,
        &["0:Library 0", "0:Library 1"]
    )]
    #[case::a_sum(2, r#"{"Mill":{"count":{"Sum":[1,{"Life":"You"}]}}}"#, &["0:Library 0", "0:Library 1", "0:Library 2", "0:Library 3", "0:Library 4"])]
    #[case::an_opponent(2, r#"{"Mill":{"who":"Opponents","count":2}}"#, &["1:Library 0", "1:Library 1"])]
    #[case::each_opponent_in_turn_order(3, r#"{"Mill":{"who":"Opponents"}}"#, &["1:Library 0", "2:Library 0"])]
    #[case::every_player_in_turn_order(
        4,
        r#"{"Mill":{"who":"EachPlayer","count":2}}"#,
        &["0:Library 0", "0:Library 1", "1:Library 0", "1:Library 1", "2:Library 0", "2:Library 1", "3:Library 0", "3:Library 1"]
    )]
    #[case::something_that_is_not_a_player(2, r#"{"Mill":{"who":"This"}}"#, &[])]
    fn a_mill_moves_the_top_cards_of_the_libraries_it_names_to_their_graveyards(
        #[case] seats: usize,
        #[case] json: &str,
        #[case] want: &[&str],
    ) {
        assert_eq!(milled(&game(seats, &[], &[]), json), want);
    }

    #[rstest]
    #[case::an_empty_library_gives_nothing(&[1], &[], r#"{"Mill":{"who":"Opponents","count":3}}"#, &["2:Library 0", "2:Library 1", "2:Library 2"])]
    #[case::every_library_empty(&[0, 1, 2], &[], r#"{"Mill":{"who":"EachPlayer","count":3}}"#, &[])]
    #[case::an_empty_library_among_others(&[1], &[], r#"{"Mill":{"who":"EachPlayer"}}"#, &["0:Library 0", "2:Library 0"])]
    #[case::a_player_who_left_is_skipped(&[], &[1], r#"{"Mill":{"who":"EachPlayer"}}"#, &["0:Library 0", "2:Library 0"])]
    #[case::my_own_empty_library(&[0], &[], r#"{"Mill":{"count":2}}"#, &[])]
    fn libraries_without_cards_and_players_who_left_give_nothing(
        #[case] empty: &[usize],
        #[case] out: &[usize],
        #[case] json: &str,
        #[case] want: &[&str],
    ) {
        assert_eq!(milled(&game(3, empty, out), json), want);
    }

    #[test]
    fn a_count_the_evaluator_cannot_read_is_refused_by_name() {
        assert_eq!(
            run_on(&game(2, &[], &[]), r#"{"Mill":{"count":"Divided"}}"#).unwrap_err(),
            ResolveError::Unsupported("a divided amount outside the damage or counters it divides")
        );
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_on(&game(2, &[], &[]), r#"{"Draw":{}}"#).unwrap_err(),
            ResolveError::Invalid("the Mill primitive was given another effect".into())
        );
    }
}
