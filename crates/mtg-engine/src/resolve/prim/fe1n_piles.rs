//! `Effect::SeparatePiles`: a player separates objects into two piles (CR 700.3) and another chooses one of them.

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{ChoiceOption, Questions, Stop, Target};
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::state::GameState;

/// The names of the objects of a pile, or a word for no card.
fn describe(state: &GameState, pile: &[ObjectId]) -> String {
    if pile.is_empty() {
        return "no cards".to_owned();
    }
    let names: Vec<String> = pile
        .iter()
        .map(|id| {
            state
                .object(*id)
                .map_or_else(String::new, |o| o.card().name.clone())
        })
        .collect();
    names.join(", ")
}

/// The first player `who` names who is in the game.
fn player_of(cx: &Cx<'_>, who: &mtg_cards::refs::Ref) -> Result<Option<PlayerId>, ResolveError> {
    Ok(cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| cx.state().is_alive(*player)))
}

/// The separator picks the cards of the first pile, the rest make the second; the chooser then picks a pile. Returns the
/// chosen pile and the other one, as objects.
fn walk(
    cx: &Cx<'_>,
    (separator, chooser): (PlayerId, PlayerId),
    objects: &[crate::object::ObjectRef],
) -> Result<(Vec<Target>, Vec<Target>), Stop> {
    let ids: Vec<ObjectId> = objects.iter().map(|object| object.id).collect();
    let mut questions = Questions::new(cx);
    let first = questions.cards(
        separator,
        ids.clone(),
        (0, ids.len()),
        "Separate the cards into two piles: choose the cards of the first pile.".to_owned(),
    )?;
    let second: Vec<ObjectId> = ids
        .iter()
        .copied()
        .filter(|id| !first.contains(id))
        .collect();
    let state = cx.state();
    let options = vec![
        ChoiceOption::Label(format!("Pile 1: {}", describe(state, &first))),
        ChoiceOption::Label(format!("Pile 2: {}", describe(state, &second))),
    ];
    let picked = questions.w9b_option(chooser, options, "Choose a pile.".to_owned())?;
    questions.end()?;
    let split = |pile: &[ObjectId]| -> Vec<Target> {
        objects
            .iter()
            .filter(|object| pile.contains(&object.id))
            .map(|object| Target::Object(*object))
            .collect()
    };
    let (chosen, rest) = if picked == 0 {
        (&first, &second)
    } else {
        (&second, &first)
    };
    Ok((split(chosen), split(rest)))
}

/// The objects `what` names are separated by the first player `who` names, and the first player `chooser` names takes a
/// pile: that pile is bound as `chosen_as` and the other as `rest_as`. Nothing to separate, or nobody to separate or to
/// choose, binds both names to nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::SeparatePiles {
        who,
        what,
        chooser,
        chosen_as,
        rest_as,
    } = effect
    else {
        return Err(wrong_effect("SeparatePiles"));
    };
    let objects = cx.scope.objects(what)?;
    let people = (player_of(cx, who)?, player_of(cx, chooser)?);
    let piles = match people {
        (Some(separator), Some(chooser)) if !objects.is_empty() => {
            match walk(cx, (separator, chooser), &objects) {
                Ok(piles) => piles,
                Err(stop) => return Stop::settle(Err(stop)),
            }
        }
        _ => (Vec::new(), Vec::new()),
    };
    Ok(Run::Done {
        events: Vec::new(),
        bound: vec![(chosen_as.clone(), piles.0), (rest_as.clone(), piles.1)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
