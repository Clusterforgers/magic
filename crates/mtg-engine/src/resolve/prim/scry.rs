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
/// cards from the top of their library (all of a shorter one) and answer one decision that splits them into the cards
/// that stay on top and those for the bottom (any number, none or all) and orders both piles. Each player is asked,
/// always, since seeing the cards is the point. A scry of nothing, or of an empty library, does not happen
/// (CR 701.22b). All the scries are decided before the library of anyone changes; each is one `Scried`.
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
        let chosen = questions.scry(*player, looked, wording::SCRY.to_owned())?;
        events.push(Event::Scried(Scried {
            player: *player,
            top: chosen.top,
            bottom: chosen.bottom,
        }));
    }
    questions.end()?;
    Ok(events)
}

#[cfg(test)]
mod tests {
    //! Goal: a scry looks at the top cards (all of a short library, none of an empty one or for a count of nothing), asks one decision per player that splits the cards into a top pile and a bottom pile and orders both, logs one `Scried` per player with both piles in the order answered, decides every player's scry before any library changes, and refuses an answer it did not ask for.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::{Choice, ScryChoice, ScryOptions};
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

    fn piles(top: &[ObjectId], bottom: &[ObjectId]) -> Choice {
        Choice::Scry(ScryChoice {
            top: top.to_vec(),
            bottom: bottom.to_vec(),
        })
    }

    #[test]
    fn the_question_is_one_scry_with_the_cards_looked_at_top_first() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), r#"{"Scry":{"count":3}}"#).unwrap());
        assert_eq!(
            decision,
            Decision {
                player: ME,
                kind: DecisionKind::Scry(ScryOptions {
                    source: game.zone(Zone::Battlefield)[0],
                    cards: top(&game, ME, 3),
                    text: "Put any number of the cards on the bottom of your library.".into(),
                })
            }
        );
    }

    #[test]
    fn one_card_is_still_asked_since_the_player_sees_it() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), r#"{"Scry":{}}"#).unwrap());
        let DecisionKind::Scry(options) = decision.kind else {
            panic!("a scry is asked");
        };
        assert_eq!(options.cards, top(&game, ME, 1));
    }

    /// For a scry of `count` cards (numbered from the top), the answered split: which stay on top and which go down,
    /// each in the order given; both piles come back as answered in one `Scried`.
    #[rstest]
    #[case::one_stays(1, &[0], &[])]
    #[case::one_goes(1, &[], &[0])]
    #[case::two_swapped_on_top(2, &[1, 0], &[])]
    #[case::two_split(2, &[1], &[0])]
    #[case::three_all_on_top_reordered(3, &[2, 0, 1], &[])]
    #[case::three_all_to_the_bottom_reordered(3, &[], &[1, 2, 0])]
    #[case::three_mixed_reordered(3, &[2, 0], &[1])]
    #[case::four_mixed_both_reordered(4, &[3, 1], &[2, 0])]
    #[case::five_all_on_top_reversed(5, &[4, 3, 2, 1, 0], &[])]
    #[case::five_all_to_the_bottom(5, &[], &[0, 1, 2, 3, 4])]
    #[case::five_mixed(5, &[4, 0, 2], &[3, 1])]
    fn one_answer_splits_the_cards_and_keeps_the_order_of_both_piles(
        #[case] count: usize,
        #[case] stay: &[usize],
        #[case] down: &[usize],
    ) {
        let game = game();
        let looked = top(&game, ME, count);
        let pick = |ix: &[usize]| ix.iter().map(|i| looked[*i]).collect::<Vec<_>>();
        let json = format!(r#"{{"Scry":{{"count":{count}}}}}"#);
        let journal = vec![(ME, piles(&pick(stay), &pick(down)))];
        let done = scried(run_with(&game, journal, &json).unwrap());
        assert_eq!(
            done,
            [Scried {
                player: ME,
                top: pick(stay),
                bottom: pick(down)
            }]
        );
    }

    #[test]
    fn a_short_library_is_looked_at_whole_and_an_empty_one_or_a_count_of_nothing_does_not_happen() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), r#"{"Scry":{"count":9}}"#).unwrap());
        let DecisionKind::Scry(options) = decision.kind else {
            panic!("a scry is asked");
        };
        assert_eq!(options.cards, top(&game, ME, 5));
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
        let journal = vec![(ME, piles(&mine[1..], &mine[..1]))];
        let second = asked(run_with(&game, journal.clone(), json).unwrap());
        let DecisionKind::Scry(options) = second.kind else {
            panic!("a scry is asked");
        };
        assert_eq!((second.player, options.cards), (FOE, theirs.clone()));
        let journal = [journal, vec![(FOE, piles(&[theirs[1], theirs[0]], &[]))]].concat();
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
            vec![(ME, piles(&[ObjectId(999)], &[]))],
            vec![(FOE, piles(&looked, &[]))],
            vec![(ME, Choice::YesNo(true))],
            vec![(ME, Choice::Order(looked.clone()))],
            vec![(ME, piles(&looked[..1], &[]))],
            vec![(ME, piles(&looked, &looked[..1]))],
            vec![(ME, piles(&looked, &[])), (ME, piles(&looked, &[]))],
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
