//! `Effect::Vote`: each player votes (CR 701.38), the controller first and the others in turn order, and the resolution
//! goes on with the tally bound to names.

use mtg_cards::effect::{Effect, VoteFor};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{
    ChoiceOption, OptionChooser, Optioned, Picked, Selection, Target, pick_options, pick_selections,
};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// The players who vote, the controller first, in turn order; a player who has left the game does not vote.
fn voters(cx: &Cx<'_>) -> Vec<PlayerId> {
    let state = cx.state();
    state.turn_order_from(cx.scope.controller)
}

/// How a name is shown in the question and in the options: the words of the card.
fn shown(name: &str) -> String {
    name.replace('_', " ")
}

/// The step is done with these names bound and goes on with the next effect.
fn done(bound: Vec<(String, Vec<Target>)>) -> Run {
    Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    }
}

/// Each voter chooses one of the names, one at a time (a secret vote is answered face down); each name is bound to the
/// players who voted for it.
fn named(cx: &Cx<'_>, names: &[String], secret: bool) -> Result<Run, ResolveError> {
    let options: Vec<ChoiceOption> = names
        .iter()
        .map(|name| {
            if secret {
                ChoiceOption::Secret(shown(name))
            } else {
                ChoiceOption::Label(shown(name))
            }
        })
        .collect();
    let text = format!(
        "Vote for {}.",
        names
            .iter()
            .map(|n| shown(n))
            .collect::<Vec<_>>()
            .join(" or ")
    );
    let voters = voters(cx);
    let choosers: Vec<OptionChooser> = voters
        .iter()
        .map(|player| OptionChooser {
            player: *player,
            options: options.clone(),
            text: text.clone(),
        })
        .collect();
    let taken = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    let bound = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let for_it = taken
                .iter()
                .filter(|(_, picked)| *picked == index)
                .map(|(player, _)| Target::Player(*player))
                .collect();
            (name.clone(), for_it)
        })
        .collect();
    Ok(done(bound))
}

/// The targets with the most votes, each once, in the order they were first voted for.
fn leaders(votes: &[Target]) -> Vec<Target> {
    let count = |target: &Target| votes.iter().filter(|vote| *vote == target).count();
    let most = votes.iter().map(count).max().unwrap_or(0);
    let mut leading: Vec<Target> = Vec::new();
    for vote in votes {
        if count(vote) == most && !leading.contains(vote) {
            leading.push(*vote);
        }
    }
    leading
}

/// Each voter chooses one of the players or objects the reference names; the ones with the most votes, all of them
/// when tied, are bound.
fn of(cx: &Cx<'_>, of: &mtg_cards::refs::Ref, bind: &str) -> Result<Run, ResolveError> {
    let candidates = cx.scope.refs(of)?;
    let players: Vec<PlayerId> = candidates.iter().filter_map(|t| t.player()).collect();
    let objects: Vec<ObjectId> = candidates
        .iter()
        .filter_map(|t| t.object())
        .map(|o| o.id)
        .collect();
    if !players.is_empty() && !objects.is_empty() {
        return Err(ResolveError::Invalid(
            "a vote is on players or on objects, not both".into(),
        ));
    }
    let seats = voters(cx);
    let votes: Vec<Target> = if candidates.is_empty() {
        Vec::new()
    } else if objects.is_empty() {
        let options: Vec<ChoiceOption> = players.iter().map(|p| ChoiceOption::Player(*p)).collect();
        let choosers: Vec<OptionChooser> = seats
            .iter()
            .map(|player| OptionChooser {
                player: *player,
                options: options.clone(),
                text: "Vote for a player.".to_owned(),
            })
            .collect();
        match pick_options(cx, &choosers)? {
            Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
            Optioned::Done(taken) => taken
                .into_iter()
                .filter_map(|(_, index)| players.get(index).copied().map(Target::Player))
                .collect(),
        }
    } else {
        let selections: Vec<Selection> = seats
            .iter()
            .map(|player| Selection {
                player: *player,
                from: objects.clone(),
                min: 1,
                max: 1,
                text: "Vote for a permanent or card.".to_owned(),
            })
            .collect();
        match pick_selections(cx, &selections)? {
            Picked::Ask(decision) => return Ok(Run::Ask(decision)),
            Picked::Done(taken) => taken
                .into_iter()
                .flat_map(|(_, cards)| cards)
                .filter_map(|id| {
                    candidates
                        .iter()
                        .find(|t| t.object().is_some_and(|o| o.id == id))
                })
                .copied()
                .collect(),
        }
    };
    Ok(done(vec![(bind.to_owned(), leaders(&votes))]))
}

/// The players vote: on named choices each name is bound to the players who voted for it, once for each of their votes;
/// on players or objects the name of the effect is bound to those with the most votes, all of them when tied.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Vote {
        options,
        secret,
        bind,
    } = effect
    else {
        return Err(wrong_effect("Vote"));
    };
    match (options, bind) {
        (VoteFor::Named(names), _) => named(cx, names, *secret),
        (VoteFor::Of(_), _) if *secret => Err(ResolveError::Invalid(
            "only a vote on named choices is secret".into(),
        )),
        (VoteFor::Of(reference), Some(bind)) => of(cx, reference, bind),
        (VoteFor::Of(_), None) => Err(ResolveError::Invalid(
            "a vote on players or objects names its winners".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a vote asks the controller first and then the others in turn order whoever is the active player, offers the named choices (face down for a secret vote), keeps every tied leader and nothing else, and refuses a secret vote on players or objects, a vote on them that names no winner and another effect, by name.
    use rstest::rstest;

    use super::*;
    use crate::decision::DecisionKind;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    fn game(seats: usize) -> GameState {
        state(
            vec![vec![]; seats],
            std::iter::once(vec![creature("Source", 1, 1)])
                .chain(std::iter::repeat_n(vec![], seats - 1))
                .collect(),
        )
    }

    fn play(game: &GameState, controller: u64, effect: &str) -> Result<Run, ResolveError> {
        let source = game
            .object(game.zone(crate::zones::Zone::Battlefield)[0])
            .unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(game, PlayerId(controller), source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(effect).unwrap())
    }

    const NAMED: &str = r#"{"Vote":{"options":{"Named":["time","knowledge"]}}}"#;

    #[rstest]
    #[case::the_active_player_controls_it(0, 0)]
    #[case::another_seat_controls_it(1, 1)]
    #[case::the_last_seat_controls_it(2, 2)]
    fn the_controller_votes_first(#[case] controller: u64, #[case] first: u64) {
        let Run::Ask(decision) = play(&game(3), controller, NAMED).unwrap() else {
            panic!("a vote asks the first voter");
        };
        assert_eq!(decision.player, PlayerId(first));
    }

    #[rstest]
    #[case::open(NAMED, false)]
    #[case::secret(
        r#"{"Vote":{"options":{"Named":["time","knowledge"]},"secret":true}}"#,
        true
    )]
    fn named_choices_are_offered_with_their_words_face_down_when_secret(
        #[case] json: &str,
        #[case] secret: bool,
    ) {
        let Run::Ask(decision) = play(&game(2), 0, json).unwrap() else {
            panic!("a vote asks the first voter");
        };
        let DecisionKind::ChooseOption(options) = decision.kind else {
            panic!("named choices are options");
        };
        let want = |word: &str| {
            if secret {
                ChoiceOption::Secret(word.to_owned())
            } else {
                ChoiceOption::Label(word.to_owned())
            }
        };
        assert_eq!(options.options, [want("time"), want("knowledge")]);
        assert_eq!(options.text, "Vote for time or knowledge.");
    }

    #[rstest]
    #[case::no_votes(&[], &[])]
    #[case::one_vote(&[3], &[3])]
    #[case::a_clear_leader(&[1, 2, 2, 3], &[2])]
    #[case::a_tie_keeps_both_in_the_order_they_were_voted_for(&[2, 1, 1, 2], &[2, 1])]
    #[case::everyone_different(&[1, 2, 3], &[1, 2, 3])]
    fn the_leaders_are_those_with_the_most_votes_each_once(
        #[case] votes: &[u64],
        #[case] leading: &[u64],
    ) {
        let targets: Vec<Target> = votes.iter().map(|n| Target::Player(PlayerId(*n))).collect();
        let want: Vec<Target> = leading
            .iter()
            .map(|n| Target::Player(PlayerId(*n)))
            .collect();
        assert_eq!(leaders(&targets), want);
    }

    #[rstest]
    #[case::secret_on_players(
        r#"{"Vote":{"options":{"Of":"EachPlayer"},"secret":true,"as":"won"}}"#
    )]
    #[case::no_name_for_the_winners(r#"{"Vote":{"options":{"Of":"EachPlayer"}}}"#)]
    fn a_vote_on_players_that_cannot_be_played_is_refused(#[case] json: &str) {
        assert!(matches!(
            play(&game(2), 0, json),
            Err(ResolveError::Invalid(_))
        ));
    }

    #[test]
    fn another_effect_is_refused_by_name() {
        assert!(matches!(
            play(&game(2), 0, r#"{"Draw":{}}"#),
            Err(ResolveError::Invalid(message)) if message.contains("Vote")
        ));
    }
}
