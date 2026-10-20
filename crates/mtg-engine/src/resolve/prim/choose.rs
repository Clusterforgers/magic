//! `Effect::Choose`: a player chooses some of a set of objects and binds the choice.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{Questions, Stop, Target};
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::wording;

/// Each player the effect names who is still in the game chooses, in turn order, between `min` and `max` of the
/// objects the reference stands for that no earlier player took; the choices are bound to the name and what was left
/// to the second one. Only objects that are still the ones named count. A player is asked when the answer can differ:
/// not when nothing is left to take, nor when they must take every object left. The question is a `ChooseCards`
/// addressed to that player.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Choose {
        who,
        from,
        count,
        bind,
        rest_as,
    } = effect
    else {
        return Err(wrong_effect("Choose"));
    };
    let state = cx.state();
    let min = usize::try_from(count.min).unwrap_or(usize::MAX);
    let max = match &count.max {
        Some(amount) => usize::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(usize::MAX),
        None => usize::MAX,
    };
    let mut left: Vec<ObjectRef> = Vec::new();
    for named in cx.scope.objects(from)? {
        let here = state
            .object(named.id)
            .is_some_and(|object| object.counter() == named.counter);
        if here && !left.contains(&named) {
            left.push(named);
        }
    }
    let choosers: Vec<_> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    Stop::settle(
        choices(cx, &choosers, left, (min, max)).map(|(chosen, rest)| {
            let objects = |list: &[ObjectRef]| list.iter().copied().map(Target::Object).collect();
            let mut bound = vec![(bind.clone(), objects(&chosen))];
            if let Some(name) = rest_as {
                bound.push((name.clone(), objects(&rest)));
            }
            Run::Done {
                events: Vec::new(),
                bound,
                snapshots: Vec::new(),
                next: Next::Sibling,
            }
        }),
    )
}

/// What each chooser takes of `left` in turn, and what is left after them.
fn choices(
    cx: &Cx<'_>,
    choosers: &[PlayerId],
    mut left: Vec<ObjectRef>,
    (min, max): (usize, usize),
) -> Result<(Vec<ObjectRef>, Vec<ObjectRef>), Stop> {
    let mut questions = Questions::new(cx);
    let mut chosen: Vec<ObjectRef> = Vec::new();
    for player in choosers {
        let (lo, hi) = (min.min(left.len()), max.min(left.len()));
        let taken = if hi == 0 {
            Vec::new()
        } else if lo == left.len() {
            // Every object left must be taken, so there is nothing to choose.
            left.iter().map(|object| object.id).collect()
        } else {
            let offered = left.iter().map(|object| object.id).collect();
            questions.cards(*player, offered, (lo, hi), wording::choose(lo, hi))?
        };
        let (now, rest): (Vec<_>, Vec<_>) =
            left.iter().partition(|object| taken.contains(&object.id));
        chosen.extend(now);
        left = rest;
    }
    questions.end()?;
    Ok((chosen, left))
}

#[cfg(test)]
mod tests {
    //! Goal: choosing takes between the fewest and the most of the objects named, asks the player only when the answer can differ (nothing left, or all of it forced, is taken without a question), binds the choice and the rest to their names, counts only objects that are still the ones named, goes through the players in turn order from what is left, and the question is a choice of cards addressed to the chooser.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice};
    use crate::decision::{Decision, DecisionKind};
    use crate::events::Event;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 holds A, B, C and D, seat 1 holds E and F; the Source is on the battlefield.
    fn game() -> GameState {
        let hand = |names: &[&str]| names.iter().map(|n| creature(n, 1, 1)).collect();
        state(
            vec![hand(&["A", "B", "C", "D"]), hand(&["E", "F"])],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn hand(game: &GameState, player: PlayerId) -> Vec<ObjectId> {
        game.zone(Zone::Hand(player)).to_vec()
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

    const MINE: &str = r#"{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]}"#;

    fn bound(run: Run) -> Vec<(String, Vec<ObjectId>)> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the choice is made");
        };
        assert!(events.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        bound
            .into_iter()
            .map(|(name, targets)| {
                (
                    name,
                    targets.iter().map(|t| t.object().unwrap().id).collect(),
                )
            })
            .collect()
    }

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the effect asks");
        };
        decision
    }

    #[test]
    fn the_player_is_asked_with_the_cards_the_bounds_and_the_words() {
        let game = game();
        let json =
            format!(r#"{{"Choose":{{"from":{MINE},"count":{{"min":2,"max":2}},"as":"back"}}}}"#);
        let decision = asked(run_with(&game, Vec::new(), &json).unwrap());
        assert_eq!(
            decision,
            Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source: game.zone(Zone::Battlefield)[0],
                    from: hand(&game, ME),
                    min: 2,
                    max: 2,
                    text: "Choose 2 cards.".into(),
                })
            }
        );
    }

    #[test]
    fn the_answer_is_bound_and_the_rest_is_bound_when_a_name_is_given() {
        let game = game();
        let cards = hand(&game, ME);
        let json = format!(
            r#"{{"Choose":{{"from":{MINE},"count":{{"min":2,"max":2}},"as":"back","rest_as":"kept"}}}}"#
        );
        let journal = vec![(ME, Choice::Cards(vec![cards[3], cards[1]]))];
        let done = bound(run_with(&game, journal, &json).unwrap());
        assert_eq!(
            done,
            [
                ("back".to_owned(), vec![cards[1], cards[3]]),
                ("kept".to_owned(), vec![cards[0], cards[2]]),
            ]
        );
    }

    #[rstest]
    #[case::exactly_all_of_them(r#"{"min":4,"max":4}"#, 4)]
    #[case::at_least_all_of_them(r#"{"min":6,"max":6}"#, 4)]
    #[case::none_at_most(r#"{"min":0,"max":0}"#, 0)]
    fn nothing_to_choose_is_taken_without_a_question(#[case] bounds: &str, #[case] taken: usize) {
        let game = game();
        let json = format!(r#"{{"Choose":{{"from":{MINE},"count":{bounds},"as":"back"}}}}"#);
        let done = bound(run_with(&game, Vec::new(), &json).unwrap());
        assert_eq!(done[0].1, hand(&game, ME)[..taken]);
    }

    #[rstest]
    #[case::up_to_all_of_them(r#"{"min":0,"max":4}"#, 0, 4, "Choose up to 4 cards.")]
    #[case::any_number(r#"{"min":0,"max":null}"#, 0, 4, "Choose up to 4 cards.")]
    #[case::one_card("{}", 1, 1, "Choose a card.")]
    #[case::a_range(r#"{"min":1,"max":3}"#, 1, 3, "Choose 1 to 3 cards.")]
    fn a_choice_that_can_differ_is_asked_with_the_bounds_clamped_to_what_there_is(
        #[case] bounds: &str,
        #[case] min: u32,
        #[case] max: u32,
        #[case] text: &str,
    ) {
        let game = game();
        let json = format!(r#"{{"Choose":{{"from":{MINE},"count":{bounds},"as":"back"}}}}"#);
        let decision = asked(run_with(&game, Vec::new(), &json).unwrap());
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(
            (options.min, options.max, options.text.as_str()),
            (min, max, text)
        );
    }

    #[test]
    fn every_player_named_chooses_in_turn_order_from_what_is_left() {
        let game = game();
        let from = r#"{"Each":[{"In":"Hand"}]}"#;
        let json = format!(r#"{{"Choose":{{"who":"EachPlayer","from":{from},"as":"picked"}}}}"#);
        let first = asked(run_with(&game, Vec::new(), &json).unwrap());
        assert_eq!(first.player, ME);
        let ours = hand(&game, ME)[0];
        let second = asked(run_with(&game, vec![(ME, Choice::Cards(vec![ours]))], &json).unwrap());
        assert_eq!(second.player, FOE);
        let DecisionKind::ChooseCards(options) = second.kind else {
            panic!("cards are chosen");
        };
        assert!(!options.from.contains(&ours), "what the first took is gone");
        assert_eq!(options.from.len(), 5);
    }

    #[test]
    fn a_stale_object_is_not_offered() {
        let mut game = game();
        let gone = hand(&game, ME)[0];
        let named = game.object(gone).unwrap().reference();
        crate::fold::apply_event(
            &mut game,
            &Event::CardDiscarded {
                player: ME,
                object: gone,
            },
        )
        .unwrap();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let mut resolving = Resolving::begin(source.id());
        let mut stale = hand(&game, ME)
            .into_iter()
            .filter_map(|id| game.object(id))
            .map(|o| Target::Object(o.reference()))
            .collect::<Vec<_>>();
        stale.push(Target::Object(named));
        resolving.bindings.insert("seen".into(), stale);
        let cx = Cx {
            scope: Scope {
                bindings: &resolving.bindings,
                ..Scope::announcing(&game, ME, source.reference())
            },
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(
            r#"{"Choose":{"from":{"Bound":"seen"},"count":{"min":0,"max":9},"as":"back"}}"#,
        )
        .unwrap();
        let decision = asked(run(&cx, &effect).unwrap());
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(options.from, hand(&game, ME));
    }

    #[test]
    fn an_answer_the_step_cannot_use_is_a_bug() {
        let game = game();
        let json =
            format!(r#"{{"Choose":{{"from":{MINE},"count":{{"min":2,"max":2}},"as":"back"}}}}"#);
        let one = hand(&game, ME)[0];
        for journal in [
            vec![(ME, Choice::Cards(vec![one]))],
            vec![(ME, Choice::Cards(vec![ObjectId(999), one]))],
            vec![(FOE, Choice::Cards(hand(&game, ME)[..2].to_vec()))],
            vec![(ME, Choice::YesNo(true))],
        ] {
            let refused = run_with(&game, journal.clone(), &json);
            assert!(
                matches!(refused, Err(ResolveError::Invalid(_))),
                "{journal:?}: {refused:?}"
            );
        }
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let refused = run_with(&game(), Vec::new(), r#"{"Draw":{}}"#);
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
