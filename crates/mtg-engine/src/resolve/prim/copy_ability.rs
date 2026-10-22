//! `Effect::CopyAbility`: copies of activated and triggered abilities on the stack are put on the stack above them
//! (CR 707.10).

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::retarget::{Original, Retargeted, retarget};
use super::wrong_effect;
use crate::copies::AbilityCopy;
use crate::events::Event;
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each ability the effect names that is still on the stack is copied `count` times, one copy after the other. A copy is
/// neither activated nor triggered: no trigger sees an activation, no cost is paid and no limit of the ability is used.
/// It has the source, rules, cause and choices of the original, and when the effect lets them its controller may choose
/// new targets for it (`retarget`). The ability the effect itself is, a spell and an object that has left the stack are
/// passed over.
///
/// A step with k questions runs k + 1 times; every copy's questions are read from the journal in the order of the copies.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CopyAbility {
        what,
        count,
        new_targets,
    } = effect
    else {
        return Err(wrong_effect("CopyAbility"));
    };
    let copies = u32::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(u32::MAX);
    let abilities: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Stack
                && object.kind() == ObjectKind::Ability
                && object.id() != cx.resolving.object()
        })
        .collect();
    let mut answers = cx.journal().iter();
    let mut planned = Vec::new();
    for object in &abilities {
        let Some(ability) = object.ability() else {
            continue;
        };
        for _ in 0..copies {
            let original = Original {
                id: object.id(),
                name: object.card().name.as_str(),
                choices: object.choices(),
                ability: ability.rules(),
                origin: ability.source(),
                cause: ability.cause(),
            };
            match retarget(cx, &mut answers, original, *new_targets)? {
                Retargeted::Ask(decision) => return Ok(Run::Ask(decision)),
                Retargeted::Done(choices) => planned.push((*object, choices)),
            }
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a copy of an ability that was not asked".into(),
        ));
    }
    let first = cx.state().next_object().get();
    let events = (first..)
        .zip(planned)
        .map(|(id, (object, choices))| {
            Event::AbilityCopied(Box::new(AbilityCopy {
                object: ObjectId(id),
                of: object.reference(),
                controller: cx.controller(),
                choices,
            }))
        })
        .collect();
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
