//! `Effect::Choose`: a player chooses some of a set of players or objects and binds what they chose.

use mtg_cards::effect::Effect;
use mtg_cards::value::Amount;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{
    ChoiceOption, OptionChooser, Optioned, Picked, Selection, Target, pick_options, pick_selections,
};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::wording;

/// The player `who` names (the controller by default) chooses between `count.min` and `count.max` of what `from`
/// names, and the choice is bound to `as`; what was not chosen is bound to `rest_as` when it has a name. Players are
/// chosen one at a time from a `ChooseOption` (a defending player, an opponent), objects from a `ChooseCards`. A
/// player with nothing to choose between (the only opponent, or as many candidates as they must take) is not asked.
/// Several players who choose each choose from the same set, and what they chose is bound together.
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
    let candidates = cx.scope.refs(from)?;
    let choosers: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| cx.state().is_alive(*player))
        .collect();
    let total = candidates.len();
    let min = at_most(cx, &count.min, total)?;
    let most = match &count.max {
        None => total,
        Some(amount) => at_most(cx, amount, total)?,
    }
    .max(min);
    let players: Vec<PlayerId> = candidates.iter().filter_map(|t| t.player()).collect();
    let objects: Vec<_> = candidates.iter().filter_map(|t| t.object()).collect();
    let mut chosen: Vec<Target> = if total == 0 {
        Vec::new()
    } else if objects.is_empty() {
        if (min, most) != (1, 1) {
            return Err(ResolveError::Invalid(
                "a choice among players takes exactly one".into(),
            ));
        }
        let options: Vec<ChoiceOption> = players.iter().map(|p| ChoiceOption::Player(*p)).collect();
        let text = wording::choose_player(from);
        let choosers: Vec<OptionChooser> = choosers
            .iter()
            .map(|player| OptionChooser {
                player: *player,
                options: options.clone(),
                text: text.clone(),
            })
            .collect();
        match pick_options(cx, &choosers)? {
            Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
            Optioned::Done(taken) => taken
                .into_iter()
                .filter_map(|(_, index)| players.get(index).copied().map(Target::Player))
                .collect(),
        }
    } else if players.is_empty() {
        let ids: Vec<ObjectId> = objects.iter().map(|o| o.id).collect();
        let text = wording::choose_objects(from, min, most);
        let selections: Vec<Selection> = choosers
            .iter()
            .map(|player| Selection {
                player: *player,
                from: ids.clone(),
                min,
                max: most,
                text: text.clone(),
            })
            .collect();
        match pick_selections(cx, &selections)? {
            Picked::Ask(decision) => return Ok(Run::Ask(decision)),
            Picked::Done(taken) => taken
                .into_iter()
                .flat_map(|(_, cards)| cards)
                .filter_map(|id| objects.iter().find(|o| o.id == id).copied())
                .map(Target::Object)
                .collect(),
        }
    } else {
        return Err(ResolveError::Invalid(
            "a choice is among players or among objects, not both".into(),
        ));
    };
    // Players who chose alike chose once.
    let mut seen = Vec::with_capacity(chosen.len());
    chosen.retain(|target| {
        let fresh = !seen.contains(target);
        seen.push(*target);
        fresh
    });
    let mut bound = vec![(bind.clone(), chosen.clone())];
    if let Some(rest) = rest_as {
        let others = candidates
            .into_iter()
            .filter(|target| !chosen.contains(target))
            .collect();
        bound.push((rest.clone(), others));
    }
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The amount as a count, no more than `total`.
fn at_most(cx: &Cx<'_>, amount: &Amount, total: usize) -> Result<usize, ResolveError> {
    let count = usize::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(usize::MAX);
    Ok(count.min(total))
}

#[cfg(test)]
mod tests {
    //! Goal: a choice binds what the player chose and, when asked, the rest: players are chosen from a choice of options and objects from a choice of cards, both in the words of the card; the player asked is the one named, anyone with nothing to choose between is not asked, bounds are kept and clamped to what there is, and a choice that mixes players with objects or takes more than one player is refused.
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);

    /// Seat 0 holds A, B and C; every seat but 0 holds nothing; the Source is on seat 0's battlefield.
    fn game(seats: usize) -> GameState {
        let mut hands = vec![vec![
            creature("A", 1, 1),
            creature("B", 1, 1),
            creature("C", 1, 1),
        ]];
        hands.extend((1..seats).map(|_| Vec::new()));
        let mut battlefield = vec![vec![creature("Source", 1, 1)]];
        battlefield.extend((1..seats).map(|_| Vec::new()));
        state(hands, battlefield)
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

    fn bound(run: Run) -> Vec<(String, Vec<Target>)> {
        match run {
            Run::Done { bound, next, .. } => {
                assert_eq!(next, Next::Sibling);
                bound
            }
            Run::Ask(decision) => panic!("nothing was to be asked: {decision:?}"),
            Run::Cast(cast) => panic!("nothing was to be cast: {cast:?}"),
        }
    }

    fn players(list: &[usize]) -> Vec<Target> {
        list.iter()
            .map(|seat| Target::Player(PlayerId::from_index(*seat)))
            .collect()
    }

    const DEFENDER: &str = r#"{"Choose":{"from":"Opponents","as":"defender"}}"#;

    #[test]
    fn the_only_opponent_is_chosen_without_a_question() {
        let done = bound(run_with(&game(2), vec![], &effect(DEFENDER)).unwrap());
        assert_eq!(done, [("defender".to_owned(), players(&[1]))]);
    }

    #[rstest]
    #[case::three_seats(3, &[1, 2])]
    #[case::four_seats(4, &[1, 2, 3])]
    fn several_opponents_are_asked_which_in_the_words_of_the_card(
        #[case] seats: usize,
        #[case] offered: &[usize],
    ) {
        let Run::Ask(decision) = run_with(&game(seats), vec![], &effect(DEFENDER)).unwrap() else {
            panic!("the controller is asked which opponent");
        };
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("a choice of option was expected");
        };
        let expected: Vec<ChoiceOption> = offered
            .iter()
            .map(|seat| ChoiceOption::Player(PlayerId::from_index(*seat)))
            .collect();
        assert_eq!(options.options, expected);
        assert_eq!(options.text, "Choose an opponent.");
    }

    #[rstest]
    #[case::the_first(0, 1)]
    #[case::the_last(1, 2)]
    fn the_option_chosen_is_what_is_bound(#[case] index: usize, #[case] seat: usize) {
        let journal = vec![(ME, Choice::Option(index))];
        let done = bound(run_with(&game(3), journal, &effect(DEFENDER)).unwrap());
        assert_eq!(done, [("defender".to_owned(), players(&[seat]))]);
    }

    #[test]
    fn another_player_chooses_when_the_effect_says_so() {
        let json = r#"{"Choose":{"who":{"Target":0},"from":"EachPlayer","as":"one"}}"#;
        // Nobody was targeted, so nobody chooses and nothing is bound but the empty name.
        let done = bound(run_with(&game(3), vec![], &effect(json)).unwrap());
        assert_eq!(done, [("one".to_owned(), Vec::new())]);
        let json = r#"{"Choose":{"who":"Opponents","from":"EachPlayer","as":"one"}}"#;
        let Run::Ask(decision) = run_with(&game(3), vec![], &effect(json)).unwrap() else {
            panic!("the opponent is asked");
        };
        assert_eq!(decision.player, PlayerId(1));
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("a choice of option was expected");
        };
        assert_eq!(options.text, "Choose a player.");
        assert_eq!(options.options.len(), 3);
    }

    const FROM_HAND: &str = r#"[{"In":"Hand"},{"OwnedBy":"You"}]"#;

    fn cards(game: &GameState) -> Vec<ObjectId> {
        game.zone(Zone::Hand(ME)).to_vec()
    }

    fn objects(game: &GameState, picks: &[usize]) -> Vec<Target> {
        let hand = cards(game);
        picks
            .iter()
            .map(|at| Target::Object(game.object(hand[*at]).unwrap().reference()))
            .collect()
    }

    #[test]
    fn cards_are_asked_for_within_the_bounds_and_the_rest_is_bound_too() {
        let game = game(2);
        let json = format!(
            r#"{{"Choose":{{"from":{{"Each":{FROM_HAND}}},"count":{{"min":2,"max":2}},"as":"back","rest_as":"kept"}}}}"#
        );
        let choose = effect(&json);
        let Run::Ask(decision) = run_with(&game, vec![], &choose).unwrap() else {
            panic!("the controller is asked");
        };
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("a choice of cards was expected");
        };
        assert_eq!(options.from, cards(&game));
        assert_eq!((options.min, options.max), (2, 2));
        assert_eq!(options.text, "Choose 2 cards in your hand.");
        let journal = vec![(ME, Choice::Cards(vec![cards(&game)[2], cards(&game)[0]]))];
        let done = bound(run_with(&game, journal, &choose).unwrap());
        assert_eq!(
            done,
            [
                ("back".to_owned(), objects(&game, &[2, 0])),
                ("kept".to_owned(), objects(&game, &[1]))
            ]
        );
    }

    #[rstest]
    #[case::the_default_is_one("", (1, 1))]
    #[case::one_of_three(r#""count":{"min":1,"max":1},"#, (1, 1))]
    #[case::up_to_two(r#""count":{"min":0,"max":2},"#, (0, 2))]
    #[case::any_number(r#""count":{"min":0,"max":null},"#, (0, 3))]
    #[case::more_than_there_are(r#""count":{"min":1,"max":9},"#, (1, 3))]
    fn the_bounds_are_clamped_to_the_cards_there_are(
        #[case] count: &str,
        #[case] expected: (u32, u32),
    ) {
        let json = format!(r#"{{"Choose":{{"from":{{"Each":{FROM_HAND}}},{count}"as":"some"}}}}"#);
        let game = game(2);
        let Run::Ask(decision) = run_with(&game, vec![], &effect(&json)).unwrap() else {
            panic!("the controller is asked");
        };
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("a choice of cards was expected");
        };
        assert_eq!((options.min, options.max), expected);
    }

    #[rstest]
    #[case::all_of_them(r#""count":{"min":3,"max":3}"#, 3)]
    #[case::more_than_there_are(r#""count":{"min":5,"max":5}"#, 3)]
    #[case::none(r#""count":{"min":0,"max":0}"#, 0)]
    fn what_there_is_no_choice_about_is_bound_without_a_question(
        #[case] count: &str,
        #[case] taken: usize,
    ) {
        let json = format!(
            r#"{{"Choose":{{"from":{{"Each":{FROM_HAND}}},{count},"as":"some","rest_as":"left"}}}}"#
        );
        let game = game(2);
        let done = bound(run_with(&game, vec![], &effect(&json)).unwrap());
        let all: Vec<usize> = (0..taken).collect();
        let rest: Vec<usize> = (taken..3).collect();
        assert_eq!(
            done,
            [
                ("some".to_owned(), objects(&game, &all)),
                ("left".to_owned(), objects(&game, &rest))
            ]
        );
    }

    #[test]
    fn nothing_to_choose_from_binds_nothing() {
        let json =
            r#"{"Choose":{"from":{"Each":[{"In":"Graveyard"}]},"as":"some","rest_as":"left"}}"#;
        let done = bound(run_with(&game(2), vec![], &effect(json)).unwrap());
        assert_eq!(
            done,
            [
                ("some".to_owned(), Vec::new()),
                ("left".to_owned(), Vec::new())
            ]
        );
    }

    #[rstest]
    #[case::two_players(
        r#"{"Choose":{"from":"EachPlayer","count":{"min":2,"max":2},"as":"x"}}"#,
        "a choice among players takes exactly one"
    )]
    #[case::players_or_no_one(
        r#"{"Choose":{"from":"EachPlayer","count":{"min":0,"max":1},"as":"x"}}"#,
        "a choice among players takes exactly one"
    )]
    fn a_choice_the_options_cannot_express_is_refused(#[case] json: &str, #[case] why: &str) {
        assert_eq!(
            run_with(&game(3), vec![], &effect(json)).unwrap_err(),
            ResolveError::Invalid(why.into())
        );
    }

    #[test]
    fn a_wrong_answer_in_the_journal_is_refused() {
        let journal = vec![(ME, Choice::Option(7))];
        assert!(matches!(
            run_with(&game(3), journal, &effect(DEFENDER)),
            Err(ResolveError::Invalid(_))
        ));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert_eq!(
            run_with(&game(2), vec![], &effect(r#"{"Draw":{}}"#)).unwrap_err(),
            ResolveError::Invalid("the Choose primitive was given another effect".into())
        );
    }
}
