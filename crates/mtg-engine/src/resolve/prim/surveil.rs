//! `Effect::Surveil`: players surveil (CR 701.25).

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{Questions, Stop};
use crate::events::{Event, ZoneCause};
use crate::library::{Surveilled, top_cards};
use crate::resolve::{Cx, ResolveError, Run};
use crate::wording;
use crate::zones::Zone;

/// Each player the effect names who is still in the game surveils the count, in turn order: they look at that many
/// cards from the top of their library (all of a shorter one), choose which go into the graveyard (any number, none
/// or all), then put the cards that stay on top back in any order. Each player is asked first which cards go,
/// always, since seeing the cards is the point; an order of fewer than two cards is no question. A surveil of
/// nothing, or of an empty library, does not happen (CR 701.25c). All the surveils are decided before the library
/// of anyone changes; each is one `Surveilled` followed by a `ZoneChanged` for each card put into the graveyard,
/// which is not milling.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Surveil { who, count } = effect else {
        return Err(wrong_effect("Surveil"));
    };
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    Stop::settle(surveils(cx, &players, wanted).map(Run::events))
}

/// The surveil of each player who has cards to look at, asking in turn.
fn surveils(cx: &Cx<'_>, players: &[PlayerId], wanted: usize) -> Result<Vec<Event>, Stop> {
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    let mut moves = Vec::new();
    for player in players {
        let looked = top_cards(cx.state(), *player, wanted);
        if looked.is_empty() {
            continue;
        }
        let chosen = questions.cards(
            *player,
            looked.clone(),
            (0, looked.len()),
            wording::SURVEIL.to_owned(),
        )?;
        let (graveyard, stay): (Vec<ObjectId>, Vec<ObjectId>) =
            looked.into_iter().partition(|c| chosen.contains(c));
        let top = if stay.len() < 2 {
            stay
        } else {
            questions.order(*player, stay, wording::library_order(false))?
        };
        moves.extend(graveyard.iter().map(|object| Event::ZoneChanged {
            object: *object,
            from: Zone::Library(*player),
            to: Zone::Graveyard(*player),
            cause: ZoneCause::Moved,
        }));
        events.push(Event::Surveilled(Surveilled {
            player: *player,
            top,
            graveyard,
        }));
    }
    questions.end()?;
    // Every library is arranged before any card leaves one.
    events.extend(moves);
    Ok(events)
}

#[cfg(test)]
mod tests {
    //! Goal: a surveil looks at the top cards (all of a short library, none of an empty one or for a count of nothing), always asks which go into the graveyard, asks the order of the cards that stay on top only when there are two or more, logs one `Surveilled` per player and a move to the graveyard (not a mill) for each card put there, decides every player's surveil before any library changes, and refuses an answer it did not ask for.
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice, OrderOptions};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, setup, state};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Two seats with five cards in each library; the Source is on the battlefield.
    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn top(game: &GameState, player: PlayerId, n: usize) -> Vec<ObjectId> {
        top_cards(game, player, n)
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        json: &str,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(json).unwrap())
    }

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the effect asks");
        };
        decision
    }

    fn done(run: Run) -> Vec<Event> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the surveil is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        events
    }

    fn cards(ids: &[ObjectId]) -> Choice {
        Choice::Cards(ids.to_vec())
    }

    fn order(ids: &[ObjectId]) -> Choice {
        Choice::Order(ids.to_vec())
    }

    fn to_graveyard(player: PlayerId, object: ObjectId) -> Event {
        Event::ZoneChanged {
            object,
            from: Zone::Library(player),
            to: Zone::Graveyard(player),
            cause: ZoneCause::Moved,
        }
    }

    #[test]
    fn the_first_question_is_which_cards_go_into_the_graveyard_with_the_cards_looked_at_top_first()
    {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), r#"{"Surveil":{"count":3}}"#).unwrap());
        assert_eq!(
            decision,
            Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source: game.zone(Zone::Battlefield)[0],
                    from: top(&game, ME, 3),
                    min: 0,
                    max: 3,
                    text: "Put any number of the cards into your graveyard.".into(),
                    can_cancel: false,
                })
            }
        );
    }

    #[rstest]
    #[case::none_to_the_graveyard(&[], &[0, 1, 2], &[], true)]
    #[case::one_to_the_graveyard(&[1], &[0, 2], &[1], true)]
    #[case::two_to_the_graveyard(&[0, 2], &[1], &[0, 2], false)]
    #[case::all_to_the_graveyard(&[0, 1, 2], &[], &[0, 1, 2], false)]
    fn the_cards_that_stay_are_ordered_only_when_there_are_two_or_more(
        #[case] picks: &[usize],
        #[case] stay: &[usize],
        #[case] gone: &[usize],
        #[case] asks_order: bool,
    ) {
        let game = game();
        let looked = top(&game, ME, 3);
        let pick = |ix: &[usize]| ix.iter().map(|i| looked[*i]).collect::<Vec<_>>();
        let json = r#"{"Surveil":{"count":3}}"#;
        let mut journal = vec![(ME, cards(&pick(picks)))];
        let mut expected_top = pick(stay);
        if asks_order {
            let decision = asked(run_with(&game, journal.clone(), json).unwrap());
            let DecisionKind::OrderCards(OrderOptions {
                cards: offered,
                text,
                ..
            }) = decision.kind
            else {
                panic!("the cards that stay are ordered");
            };
            assert_eq!((decision.player, offered), (ME, expected_top.clone()));
            assert_eq!(text, "Put the cards on top of your library in any order.");
            expected_top.reverse();
            journal.push((ME, order(&expected_top)));
        }
        let events = done(run_with(&game, journal, json).unwrap());
        let mut expected = vec![Event::Surveilled(Surveilled {
            player: ME,
            top: expected_top,
            graveyard: pick(gone),
        })];
        expected.extend(pick(gone).into_iter().map(|c| to_graveyard(ME, c)));
        assert_eq!(events, expected);
    }

    #[rstest]
    #[case::a_short_library_is_looked_at_whole(r#"{"Surveil":{"count":9}}"#, 5)]
    #[case::one_card_is_still_asked(r#"{"Surveil":{}}"#, 1)]
    fn the_cards_looked_at_are_the_top_ones_and_the_player_always_sees_them(
        #[case] json: &str,
        #[case] looked: usize,
    ) {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), json).unwrap());
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(
            (options.from, options.min, options.max),
            (top(&game, ME, looked), 0, u32::try_from(looked).unwrap())
        );
    }

    #[test]
    fn a_count_of_nothing_or_an_empty_library_does_not_happen() {
        let game = game();
        for json in [r#"{"Surveil":{"count":0}}"#, r#"{"Surveil":{"count":-3}}"#] {
            assert_eq!(done(run_with(&game, Vec::new(), json).unwrap()), []);
        }
        let mut empty = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        empty.players[0].library.clear();
        let game = GameState::from_setup(&empty).unwrap();
        assert_eq!(
            done(run_with(&game, Vec::new(), r#"{"Surveil":{"count":2}}"#).unwrap()),
            []
        );
    }

    #[test]
    fn each_player_surveils_in_turn_order_and_no_card_moves_before_all_have_chosen() {
        let game = game();
        let json = r#"{"Surveil":{"who":"EachPlayer","count":2}}"#;
        let mine = top(&game, ME, 2);
        let theirs = top(&game, FOE, 2);
        let first = asked(run_with(&game, Vec::new(), json).unwrap());
        assert_eq!(first.player, ME);
        // Seat 0 puts one into the graveyard; seat 1 is asked next.
        let journal = vec![(ME, cards(&mine[..1]))];
        let second = asked(run_with(&game, journal.clone(), json).unwrap());
        assert_eq!(second.player, FOE);
        let journal = [journal, vec![(FOE, cards(&[]))]].concat();
        let third = asked(run_with(&game, journal.clone(), json).unwrap());
        let DecisionKind::OrderCards(options) = third.kind else {
            panic!("the cards that stay are ordered");
        };
        assert_eq!((third.player, options.cards), (FOE, theirs.clone()));
        let journal = [journal, vec![(FOE, order(&[theirs[1], theirs[0]]))]].concat();
        assert_eq!(
            done(run_with(&game, journal, json).unwrap()),
            [
                Event::Surveilled(Surveilled {
                    player: ME,
                    top: vec![mine[1]],
                    graveyard: vec![mine[0]]
                }),
                Event::Surveilled(Surveilled {
                    player: FOE,
                    top: vec![theirs[1], theirs[0]],
                    graveyard: Vec::new()
                }),
                to_graveyard(ME, mine[0]),
            ]
        );
    }

    #[test]
    fn an_answer_the_step_cannot_use_is_a_bug() {
        let game = game();
        let looked = top(&game, ME, 2);
        for journal in [
            vec![(ME, cards(&[ObjectId(999)]))],
            vec![(FOE, cards(&[]))],
            vec![(ME, Choice::YesNo(true))],
            vec![(ME, cards(&[])), (ME, order(&[looked[0]]))],
            vec![(ME, cards(&[looked[0]])), (ME, order(&[looked[1]]))],
            vec![
                (ME, cards(&[])),
                (ME, order(&[looked[1], looked[0]])),
                (ME, cards(&[])),
            ],
        ] {
            let refused = run_with(&game, journal.clone(), r#"{"Surveil":{"count":2}}"#);
            assert!(
                matches!(refused, Err(ResolveError::Invalid(_))),
                "{journal:?}: {refused:?}"
            );
        }
    }

    #[test]
    fn a_player_who_has_left_the_game_does_not_surveil() {
        let mut game = game();
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let events =
            done(run_with(&game, Vec::new(), r#"{"Surveil":{"who":"Opponents"}}"#).unwrap());
        assert_eq!(events, []);
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let refused = run_with(&game(), Vec::new(), r#"{"Draw":{}}"#);
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
