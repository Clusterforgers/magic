//! `Effect::ChangeTargets`: the targets of spells and abilities on the stack change (CR 115.7).

use mtg_cards::effect::{Effect, NewTargets};

use super::retarget::{Original, Retargeted, Way, ask};
use super::wrong_effect;
use crate::events::Event;
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::retargeted::TargetsChanged;
use crate::zones::Zone;

/// Each spell or ability the effect names that is still on the stack gets other targets, chosen by the controller of the
/// effect. The new targets must be legal for the controller of the spell or ability (CR 115.7). "Change the target" takes
/// effect only on an object with a single target and leaves it when no other target is legal; "choose new targets" asks
/// first and then for each specification that has another choice. The ability the effect itself is, and an object that
/// has left the stack, are passed over.
///
/// A step with k questions runs k + 1 times; the questions of every object are read from the journal in order.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ChangeTargets { what, to } = effect else {
        return Err(wrong_effect("ChangeTargets"));
    };
    let way = match to {
        NewTargets::Another => Way::Another,
        NewTargets::Choose => Way::Choose,
    };
    let state = cx.state();
    let chooser = cx.controller();
    let objects: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Stack && object.id() != cx.resolving.object())
        .collect();
    let mut answers = cx.journal().iter();
    let mut events = Vec::new();
    for object in &objects {
        let spell_rules = (object.kind() != ObjectKind::Ability)
            .then(|| crate::resolve::spell_ability(state, object.id()))
            .flatten();
        let (ability, origin, cause) = match (&spell_rules, object.ability()) {
            (Some((_, rules)), _) => (&**rules, object.reference(), None),
            (None, Some(ability)) if object.kind() == ObjectKind::Ability => {
                (ability.rules(), ability.source(), ability.cause())
            }
            _ => continue,
        };
        let original = Original {
            id: object.id(),
            name: object.card().name.as_str(),
            choices: object.choices(),
            ability,
            origin,
            cause,
        };
        let controller = object.controller();
        let choices = match ask(
            cx,
            (chooser, controller),
            &mut answers,
            original,
            object.choices().clone(),
            way,
        )? {
            Retargeted::Ask(decision) => return Ok(Run::Ask(decision)),
            Retargeted::Done(choices) => choices,
        };
        if choices.targets != object.choices().targets {
            let before: Vec<_> = object.choices().targets.iter().flatten().collect();
            let added = choices
                .targets
                .iter()
                .flatten()
                .filter(|target| !before.contains(target))
                .copied()
                .collect();
            events.push(Event::TargetsChanged(Box::new(TargetsChanged {
                object: object.reference(),
                controller,
                choices,
                added,
            })));
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a change of targets that was not asked".into(),
        ));
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
