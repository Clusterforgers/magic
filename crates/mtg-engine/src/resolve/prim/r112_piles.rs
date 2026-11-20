//! `Effect::R112Piles`: players separate objects into named piles (CR 700.3), some of them face down, and a pile of each
//! separator may be chosen at random or by another player (Whims of the Fates, The Celestial Toymaker, Raging River).

use mtg_cards::effect::{Effect, R112PilePick};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{ChoiceOption, Questions, Stop, Target};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::rng::GameRng;
use crate::state::GameState;

/// How a pile reads to a chooser: its name and its cards, or only how many when it is face down.
fn describe(state: &GameState, name: &str, pile: &[ObjectId], face_down: bool) -> String {
    let cards = if face_down {
        match pile.len() {
            1 => "1 face-down card".to_owned(),
            count => format!("{count} face-down cards"),
        }
    } else if pile.is_empty() {
        "nothing".to_owned()
    } else {
        pile.iter()
            .map(|id| {
                state
                    .object(*id)
                    .map_or_else(String::new, |o| o.card().name.clone())
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!("Pile \"{}\": {cards}", name.replace('_', " "))
}

/// The piles of one separator, in the order of the names: each but the last asked of the separator from the objects
/// left, the last the rest.
fn separate(
    questions: &mut Questions<'_>,
    separator: PlayerId,
    ids: &[ObjectId],
    piles: &[String],
) -> Result<Vec<Vec<ObjectId>>, Stop> {
    let mut left: Vec<ObjectId> = ids.to_vec();
    let mut separated = Vec::with_capacity(piles.len());
    for (at, name) in piles.iter().enumerate() {
        if at + 1 == piles.len() || left.is_empty() {
            separated.push(std::mem::take(&mut left));
            continue;
        }
        let pile = questions.cards(
            separator,
            left.clone(),
            (0, left.len()),
            format!(
                "Separate into {} piles: choose the cards of the pile \"{name}\".",
                piles.len()
            ),
        )?;
        left.retain(|id| !pile.contains(id));
        separated.push(pile);
    }
    separated.resize(piles.len(), Vec::new());
    Ok(separated)
}

/// What the step found: each separator's piles and, with a pick, the index of the pile chosen of each.
type Walked = Vec<(Vec<Vec<ObjectId>>, Option<usize>)>;

/// Asks the separations, then the chooser's picks; the random picks are drawn after.
fn walk(
    cx: &Cx<'_>,
    groups: &[(PlayerId, Vec<ObjectId>)],
    effect: (
        &[String],
        &[String],
        Option<(&R112PilePick, Option<PlayerId>)>,
    ),
) -> Result<Walked, Stop> {
    let (piles, face_down, pick) = effect;
    let mut questions = Questions::new(cx);
    let mut walked = Vec::with_capacity(groups.len());
    for (separator, ids) in groups {
        walked.push((separate(&mut questions, *separator, ids, piles)?, None));
    }
    if let Some((R112PilePick::By(_), Some(chooser))) = pick {
        for (separated, picked) in &mut walked {
            let options = piles
                .iter()
                .zip(separated.iter())
                .map(|(name, pile)| {
                    ChoiceOption::Label(describe(cx.state(), name, pile, face_down.contains(name)))
                })
                .collect();
            *picked = Some(questions.w9b_option(chooser, options, "Choose a pile.".to_owned())?);
        }
    }
    questions.end()?;
    Ok(walked)
}

/// The objects of `what` each separator holds: those they control (a card off the battlefield, its owner).
fn groups(
    cx: &Cx<'_>,
    separators: &[PlayerId],
    objects: &[ObjectRef],
) -> Vec<(PlayerId, Vec<ObjectId>)> {
    let state = cx.state();
    separators
        .iter()
        .map(|separator| {
            let ids = objects
                .iter()
                .filter(|object| {
                    state
                        .object(object.id)
                        .is_some_and(|o| o.controller() == *separator)
                })
                .map(|object| object.id)
                .collect();
            (*separator, ids)
        })
        .collect()
}

/// Each separator separates their objects into the piles, a pile of each is chosen when the effect picks one, and the
/// piles and the chosen piles are bound; a random pick is logged as `ChosenAtRandom` of the cards it chose.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R112Piles {
        who,
        what,
        piles,
        face_down,
        pick,
        chosen_as,
    } = effect
    else {
        return Err(wrong_effect("R112Piles"));
    };
    let state = cx.state();
    let named = cx.scope.players(who)?;
    let separators: Vec<PlayerId> = state
        .turn_order_from(cx.scope.controller)
        .into_iter()
        .filter(|player| named.contains(player) && state.is_alive(*player))
        .collect();
    let objects = cx.scope.objects(what)?;
    let groups = groups(cx, &separators, &objects);
    let chooser = match pick {
        Some(R112PilePick::By(chooser)) => cx
            .scope
            .players(chooser)?
            .into_iter()
            .find(|player| state.is_alive(*player)),
        _ => None,
    };
    let mut walked = match walk(
        cx,
        &groups,
        (piles, face_down, pick.as_ref().map(|pick| (pick, chooser))),
    ) {
        Ok(walked) => walked,
        Err(stop) => return Stop::settle(Err(stop)),
    };
    let mut events = Vec::new();
    if !walked.is_empty() {
        events.push(Event::R112Grouped {
            object: cx.resolving.object,
        });
    }
    if matches!(pick, Some(R112PilePick::Random)) && !walked.is_empty() {
        let mut rng = GameRng::substream(state.seed, state.rng_index);
        let count = u64::try_from(piles.len()).unwrap_or(1);
        let mut cards = Vec::new();
        for (separated, picked) in &mut walked {
            let at = usize::try_from(rng.below(count)).unwrap_or(0);
            cards.extend(separated.get(at).cloned().unwrap_or_default());
            *picked = Some(at);
        }
        events.push(Event::ChosenAtRandom { cards });
    }
    let target = |id: &ObjectId| {
        objects
            .iter()
            .find(|object| object.id == *id)
            .map(|object| Target::Object(*object))
    };
    let mut bound: Vec<(String, Vec<Target>)> = piles
        .iter()
        .enumerate()
        .map(|(at, name)| {
            let pile = walked
                .iter()
                .filter_map(|(separated, _)| separated.get(at))
                .flatten()
                .filter_map(target)
                .collect();
            (name.clone(), pile)
        })
        .collect();
    if let Some(name) = chosen_as {
        let chosen = walked
            .iter()
            .filter_map(|(separated, picked)| picked.and_then(|at| separated.get(at)))
            .flatten()
            .filter_map(target)
            .collect();
        bound.push((name.clone(), chosen));
    }
    Ok(Run::Done {
        events,
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The selector with each `Is` of a bound name written as the objects the name holds now (CR 611.2c): a pile named in
/// "can't be blocked except by creatures in a pile" stays the pile it was as the effect began.
pub(crate) fn r112_fix_bound(
    scope: &crate::resolve::Scope<'_>,
    selector: &mtg_cards::refs::Selector,
) -> mtg_cards::refs::Selector {
    fn fix(scope: &crate::resolve::Scope<'_>, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                let bound = fields
                    .get("Is")
                    .and_then(|is| is.get("Bound"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                if fields.len() == 1
                    && let Some(name) = bound
                    && let Ok(objects) = scope.objects(&mtg_cards::refs::Ref::Bound(name))
                {
                    let each: Vec<serde_json::Value> = objects
                        .iter()
                        .map(|object| serde_json::json!({ "Is": { "R111Object": object.id } }))
                        .collect();
                    *value = serde_json::json!({ "AnyOf": each });
                    return;
                }
                for inner in fields.values_mut() {
                    fix(scope, inner);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    fix(scope, item);
                }
            }
            _ => {}
        }
    }
    let Ok(mut value) = serde_json::to_value(selector) else {
        return selector.clone();
    };
    fix(scope, &mut value);
    serde_json::from_value(value).unwrap_or_else(|_| selector.clone())
}
