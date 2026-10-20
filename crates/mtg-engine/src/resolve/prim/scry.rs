//! `Effect::Scry`: players scry (CR 701.22).

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{Questions, Stop};
use crate::events::Event;
use crate::library::{Scried, top_cards};
use crate::resolve::{Cx, ResolveError, Run};
use crate::wording;

/// Each player the effect names who is still in the game scries the count, in turn order: they look at that many
/// cards from the top of their library (all of a shorter one), choose which go to the bottom (any number, none or
/// all), then put the cards that stay on top back in any order and the cards for the bottom in any order. Each
/// player is asked first which cards go to the bottom, always, since seeing the cards is the point; an order of
/// fewer than two cards is no question. A scry of nothing, or of an empty library, does not happen (CR 701.22b). All
/// the scries are decided before the library of anyone changes; each is one `Scried`.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Scry { who, count } = effect else {
        return Err(wrong_effect("Scry"));
    };
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    Stop::settle(scries(cx, &players, wanted).map(Run::events))
}

/// The scry of each player who has cards to look at, asking in turn.
fn scries(cx: &Cx<'_>, players: &[PlayerId], wanted: usize) -> Result<Vec<Event>, Stop> {
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    for player in players {
        let looked = top_cards(cx.state(), *player, wanted);
        if looked.is_empty() {
            continue;
        }
        let chosen = questions.cards(
            *player,
            looked.clone(),
            (0, looked.len()),
            wording::SCRY.to_owned(),
        )?;
        let (down, stay): (Vec<_>, Vec<_>) = looked.into_iter().partition(|c| chosen.contains(c));
        let top = in_any_order(&mut questions, *player, stay, false)?;
        let bottom = in_any_order(&mut questions, *player, down, true)?;
        events.push(Event::Scried(Scried {
            player: *player,
            top,
            bottom,
        }));
    }
    questions.end()?;
    Ok(events)
}

/// The cards in the order their owner puts them in; fewer than two have no order to choose.
fn in_any_order(
    questions: &mut Questions<'_>,
    player: PlayerId,
    cards: Vec<mtg_core::ids::ObjectId>,
    bottom: bool,
) -> Result<Vec<mtg_core::ids::ObjectId>, Stop> {
    if cards.len() < 2 {
        return Ok(cards);
    }
    questions.order(player, cards, wording::library_order(bottom))
}

#[cfg(test)]
mod tests {
    //! Goal: a scry looks at the top cards (all of a short library, none of an empty one or for a count of nothing), always asks which go to the bottom, asks the order of the cards that stay on top and of those for the bottom only when there are two or more, logs one `Scried` per player with both lists from the top down, decides every player's scry before any library changes, and refuses an answer it did not ask for.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice, OrderOptions};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, setup, state};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

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

    fn scried(run: Run) -> Vec<Scried> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the scry is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        events
            .into_iter()
            .map(|event| match event {
                Event::Scried(scried) => scried,
                other => panic!("only scries were expected, got {other:?}"),
            })
            .collect()
    }

    fn cards(ids: &[ObjectId]) -> Choice {
        Choice::Cards(ids.to_vec())
    }

    fn order(ids: &[ObjectId]) -> Choice {
        Choice::Order(ids.to_vec())
    }

    #[test]
    fn the_first_question_is_which_cards_go_to_the_bottom_with_the_cards_looked_at_top_first() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), r#"{"Scry":{"count":3}}"#).unwrap());
        assert_eq!(
            decision,
            Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source: game.zone(Zone::Battlefield)[0],
                    from: top(&game, ME, 3),
                    min: 0,
                    max: 3,
                    text: "Put any number of the cards on the bottom of your library.".into(),
                })
            }
        );
    }

    #[test]
    fn one_card_is_still_asked_since_the_player_sees_it() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), r#"{"Scry":{}}"#).unwrap());
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(
            (options.from, options.min, options.max),
            (top(&game, ME, 1), 0, 1)
        );
    }

    #[rstest]
    #[case::none_to_the_bottom(&[], &[0, 1, 2], &[], true, false)]
    #[case::one_to_the_bottom(&[1], &[0, 2], &[1], true, false)]
    #[case::two_to_the_bottom(&[0, 2], &[1], &[0, 2], false, true)]
    #[case::all_to_the_bottom(&[0, 1, 2], &[], &[0, 1, 2], false, true)]
    fn the_cards_that_stay_and_those_that_go_are_ordered_only_when_there_are_two_or_more(
        #[case] bottom_picks: &[usize],
        #[case] stay: &[usize],
        #[case] down: &[usize],
        #[case] asks_top: bool,
        #[case] asks_bottom: bool,
    ) {
        let game = game();
        let looked = top(&game, ME, 3);
        let pick = |ix: &[usize]| ix.iter().map(|i| looked[*i]).collect::<Vec<_>>();
        let json = r#"{"Scry":{"count":3}}"#;
        let mut journal = vec![(ME, cards(&pick(bottom_picks)))];
        let mut expected_top = pick(stay);
        let mut expected_bottom = pick(down);
        let next = run_with(&game, journal.clone(), json).unwrap();
        if asks_top {
            let decision = asked(next);
            let DecisionKind::OrderCards(OrderOptions {
                cards: offered,
                text,
                ..
            }) = decision.kind
            else {
                panic!("the cards that stay are ordered");
            };
            assert_eq!(
                (decision.player, offered.clone()),
                (ME, expected_top.clone())
            );
            assert_eq!(text, "Put the cards on top of your library in any order.");
            expected_top.reverse();
            journal.push((ME, order(&expected_top)));
        } else {
            // Nothing to order on top, or one card: the next thing is the bottom order or the end.
            assert_eq!(
                matches!(next, Run::Ask(_)),
                asks_bottom,
                "the next question is the order of the bottom cards"
            );
        }
        let next = run_with(&game, journal.clone(), json).unwrap();
        if asks_bottom {
            let decision = asked(next);
            let DecisionKind::OrderCards(OrderOptions {
                cards: offered,
                text,
                ..
            }) = decision.kind
            else {
                panic!("the cards for the bottom are ordered");
            };
            assert_eq!(offered, expected_bottom);
            assert_eq!(
                text,
                "Put the cards on the bottom of your library in any order."
            );
            expected_bottom.reverse();
            journal.push((ME, order(&expected_bottom)));
        }
        let done = scried(run_with(&game, journal, json).unwrap());
        assert_eq!(
            done,
            [Scried {
                player: ME,
                top: expected_top,
                bottom: expected_bottom
            }]
        );
    }

    #[test]
    fn a_short_library_is_looked_at_whole_and_an_empty_one_or_a_count_of_nothing_does_not_happen() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), r#"{"Scry":{"count":9}}"#).unwrap());
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(options.from, top(&game, ME, 5));
        assert_eq!(
            scried(run_with(&game, Vec::new(), r#"{"Scry":{"count":0}}"#).unwrap()),
            []
        );
        assert_eq!(
            scried(run_with(&game, Vec::new(), r#"{"Scry":{"count":-3}}"#).unwrap()),
            []
        );
        let mut empty = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        empty.players[0].library.clear();
        let game = GameState::from_setup(&empty).unwrap();
        assert_eq!(
            scried(run_with(&game, Vec::new(), r#"{"Scry":{"count":2}}"#).unwrap()),
            []
        );
    }

    #[test]
    fn each_player_scries_in_turn_order_and_nothing_changes_before_all_have_chosen() {
        let game = game();
        let json = r#"{"Scry":{"who":"EachPlayer","count":2}}"#;
        let mine = top(&game, ME, 2);
        let theirs = top(&game, FOE, 2);
        let first = asked(run_with(&game, Vec::new(), json).unwrap());
        assert_eq!(first.player, ME);
        // Seat 0 puts one on the bottom; seat 1 is asked next.
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
        let done = scried(run_with(&game, journal, json).unwrap());
        assert_eq!(
            done,
            [
                Scried {
                    player: ME,
                    top: vec![mine[1]],
                    bottom: vec![mine[0]]
                },
                Scried {
                    player: FOE,
                    top: vec![theirs[1], theirs[0]],
                    bottom: Vec::new()
                }
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
            vec![
                (ME, cards(&[])),
                (ME, order(&[looked[1], looked[0]])),
                (ME, cards(&[])),
            ],
        ] {
            let refused = run_with(&game, journal.clone(), r#"{"Scry":{"count":2}}"#);
            assert!(
                matches!(refused, Err(ResolveError::Invalid(_))),
                "{journal:?}: {refused:?}"
            );
        }
    }

    #[test]
    fn a_player_who_has_left_the_game_does_not_scry() {
        let mut game = game();
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let done = scried(run_with(&game, Vec::new(), r#"{"Scry":{"who":"Opponents"}}"#).unwrap());
        assert_eq!(done, []);
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let refused = run_with(&game(), Vec::new(), r#"{"Draw":{}}"#);
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
