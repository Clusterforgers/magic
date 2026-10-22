//! `Effect::Bolster`: the player bolsters (CR 701.39).

use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use super::put_counters::whole;
use super::wrong_effect;
use crate::characteristics::characteristics;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, ResolveError, Run};

/// The player puts the counters on the creature with the least toughness among the creatures they control, as the
/// layers have left it: with several tied for it they choose one (`ChooseCards`, asked only then). The counters are a
/// proposed `CountersChanged` that a replacement effect sees, and one `Bolstered` follows them, also when there are
/// none. A player who controls no creature bolsters nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Bolster { count } = effect else {
        return Err(wrong_effect("Bolster"));
    };
    let delta = whole(cx.scope.amount(count)?);
    Stop::settle(bolster(cx, delta))
}

/// The creatures the player controls that tie for the least toughness.
fn weakest(cx: &Cx<'_>) -> Result<Vec<ObjectRef>, ResolveError> {
    let selector = Selector(vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
    ]);
    let state = cx.state();
    let toughness =
        |object: &GameObject| characteristics(state, object.id()).and_then(|chars| chars.toughness);
    let creatures = cx.scope.live_objects(&Ref::Each(selector))?;
    let least = creatures.iter().filter_map(|c| toughness(c)).min();
    Ok(creatures
        .into_iter()
        .filter(|creature| least.is_some() && toughness(creature) == least)
        .map(GameObject::reference)
        .collect())
}

fn bolster(cx: &Cx<'_>, delta: i32) -> Result<Run, Stop> {
    let player = cx.controller();
    let tied = weakest(cx)?;
    let mut questions = Questions::new(cx);
    let chosen = match tied.as_slice() {
        [] => return Ok(Run::events(Vec::new())),
        [only] => *only,
        _ => {
            let from: Vec<ObjectId> = tied.iter().map(|creature| creature.id).collect();
            let text = "Choose a creature with the least toughness among the creatures you control to put the counters on.".to_owned();
            let taken = questions.cards(player, from, (1, 1), text)?;
            taken
                .first()
                .and_then(|id| tied.iter().find(|creature| creature.id == *id))
                .copied()
                .ok_or_else(|| ResolveError::Invalid("no creature was chosen".into()))?
        }
    };
    questions.end()?;
    let mut events = Vec::new();
    if delta > 0 {
        events.push(Event::CountersChanged(CountersChanged {
            on: Target::Object(chosen),
            counter: CounterKind::PlusOnePlusOne,
            delta,
        }));
    }
    events.push(Event::Bolstered {
        object: chosen.id,
        player,
    });
    Ok(Run::events(events))
}
