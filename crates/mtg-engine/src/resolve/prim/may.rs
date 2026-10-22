//! `Effect::May`: a player may do the effects.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::{Said, Target, pick_yes_no};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::wording;

/// The name under which the `May` at `position` binds whether it was accepted, which a following `If` of `Performed`
/// reads ("you may draw a card. If you do, discard a card"): the controller when it was, nothing when it was not. A
/// position runs once in a resolution, so the name is bound once, and the step that binds it is in the log.
pub(super) fn performed_name(position: &[usize]) -> String {
    let path: Vec<String> = position.iter().map(ToString::to_string).collect();
    format!("?performed {}", path.join("."))
}

/// The players the effect names who are still in the game are asked in turn order whether they will, until one says
/// yes: the controller by default, any seat otherwise. The effects then run; with no yes (or nobody to ask) they are
/// skipped. The question reads as the effects do ("Draw a card?") when the words for them exist. Whether they were
/// accepted is bound for an `If` that asks "if you do" (`performed_name`).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::May { who, effects } = effect else {
        return Err(wrong_effect("May"));
    };
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| cx.state().is_alive(*player))
        .collect();
    Ok(match pick_yes_no(cx, &players, &wording::may(effects))? {
        Said::Ask(decision) => Run::Ask(decision),
        Said::Done(yes) => Run::Done {
            events: Vec::new(),
            bound: vec![(
                performed_name(cx.resolving.position()),
                if yes {
                    vec![Target::Player(cx.scope.controller)]
                } else {
                    Vec::new()
                },
            )],
            snapshots: Vec::new(),
            next: if yes { Next::Enter(0) } else { Next::Sibling },
        },
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a "may" asks the player it names (the controller by default, any seat otherwise) the question in the words of its effects, goes into its effects on a yes and past them on a no, asks the players it names in turn order until one says yes, asks nobody who has left the game, and refuses another effect by name.
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    fn game(seats: usize) -> GameState {
        let mut battlefield = vec![vec![creature("Source", 1, 1)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        state(vec![Vec::new(); seats], battlefield)
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
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
        run(&cx, effect)
    }

    fn question(run: Run) -> (PlayerId, String) {
        let Run::Ask(decision) = run else {
            panic!("a question was expected");
        };
        let DecisionKind::YesNo(options) = decision.kind else {
            panic!("a yes-or-no question was expected");
        };
        (decision.player, options.text)
    }

    fn went(run: Run) -> Next {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the effect was expected to be done");
        };
        assert!(events.is_empty() && snapshots.is_empty());
        // The only thing bound is whether the effects were accepted, under the name of the position.
        assert_eq!(bound.len(), 1);
        let (name, accepted) = &bound[0];
        assert_eq!(*name, performed_name(&[0]));
        assert_eq!(!accepted.is_empty(), matches!(next, Next::Enter(_)));
        next
    }

    #[rstest]
    #[case::draw(r#"{"May":{"effects":[{"Draw":{}}]}}"#, "Draw a card?")]
    #[case::two_cards(r#"{"May":{"effects":[{"Draw":{"count":2}}]}}"#, "Draw 2 cards?")]
    #[case::two_things(
        r#"{"May":{"effects":[{"Draw":{}},{"GainLife":{"amount":3}}]}}"#,
        "Draw a card and gain 3 life?"
    )]
    #[case::three_things(
        r#"{"May":{"effects":[{"Draw":{}},{"Discard":{}},{"LoseLife":{"amount":1}}]}}"#,
        "Draw a card, discard a card and lose 1 life?"
    )]
    #[case::no_words_for_the_effect_yet(r#"{"May":{"effects":[{"Untap":{"what":"This"}}]}}"#, "")]
    #[case::done_by_someone_else(r#"{"May":{"effects":[{"Draw":{"who":"Opponents"}}]}}"#, "")]
    fn the_controller_is_asked_in_the_words_of_the_effects(#[case] json: &str, #[case] text: &str) {
        let asked = question(run_with(&game(2), vec![], &effect(json)).unwrap());
        assert_eq!(asked, (ME, text.to_owned()));
    }

    #[test]
    fn the_player_the_effect_names_is_asked_not_the_controller() {
        let may = effect(r#"{"May":{"who":"Opponents","effects":[{"Draw":{}}]}}"#);
        assert_eq!(
            question(run_with(&game(2), vec![], &may).unwrap()).0,
            PlayerId(1)
        );
    }

    #[rstest]
    #[case::yes(true, Next::Enter(0))]
    #[case::no(false, Next::Sibling)]
    fn the_answer_decides_whether_the_effects_are_entered(
        #[case] yes: bool,
        #[case] expected: Next,
    ) {
        let may = effect(r#"{"May":{"effects":[{"Draw":{}}]}}"#);
        let journal = vec![(ME, Choice::YesNo(yes))];
        assert_eq!(went(run_with(&game(2), journal, &may).unwrap()), expected);
    }

    #[rstest]
    #[case::the_first_says_yes(&[(1, true)], Some(Next::Enter(0)), None)]
    #[case::the_first_says_no_so_the_second_is_asked(&[(1, false)], None, Some(2))]
    #[case::the_second_says_yes(&[(1, false), (2, true)], Some(Next::Enter(0)), None)]
    #[case::all_say_no(&[(1, false), (2, false), (3, false)], Some(Next::Sibling), None)]
    #[case::the_first_says_no_the_second_no_the_third_is_asked(&[(1, false), (2, false)], None, Some(3))]
    fn several_players_are_asked_in_turn_order_until_one_says_yes(
        #[case] given: &[(usize, bool)],
        #[case] done: Option<Next>,
        #[case] asked: Option<usize>,
    ) {
        let may = effect(r#"{"May":{"who":"Opponents","effects":[{"Draw":{}}]}}"#);
        let journal = given
            .iter()
            .map(|(seat, yes)| (PlayerId::from_index(*seat), Choice::YesNo(*yes)))
            .collect();
        match (run_with(&game(4), journal, &may).unwrap(), done, asked) {
            (run @ Run::Done { .. }, Some(next), None) => assert_eq!(went(run), next),
            (run @ Run::Ask(_), None, Some(seat)) => {
                assert_eq!(question(run).0, PlayerId::from_index(seat));
            }
            (other, ..) => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn nobody_is_asked_when_nobody_is_left_to_decide() {
        // A target that was never chosen names nobody; the effects are skipped.
        let may = effect(r#"{"May":{"who":{"Target":0},"effects":[{"Draw":{}}]}}"#);
        assert_eq!(
            went(run_with(&game(2), vec![], &may).unwrap()),
            Next::Sibling
        );
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        let error = run_with(&game(2), vec![], &effect(r#"{"Draw":{}}"#)).unwrap_err();
        assert_eq!(
            error,
            ResolveError::Invalid("the May primitive was given another effect".into())
        );
    }
}
