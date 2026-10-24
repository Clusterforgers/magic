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
        names.iter().map(|n| shown(n)).collect::<Vec<_>>().join(" or ")
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
    let voters = voters(cx);
    let votes: Vec<Target> = if candidates.is_empty() {
        Vec::new()
    } else if objects.is_empty() {
        let options: Vec<ChoiceOption> = players.iter().map(|p| ChoiceOption::Player(*p)).collect();
        let choosers: Vec<OptionChooser> = voters
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
        let selections: Vec<Selection> = voters
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
                .filter_map(|id| candidates.iter().find(|t| t.object().is_some_and(|o| o.id == id)))
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
