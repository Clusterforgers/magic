//! `Effect::CopySpell`: copies of spells on the stack are put on the stack above them (CR 707.10).

use std::sync::Arc;

use mtg_cards::effect::Effect;
use mtg_core::ids::ObjectId;

use super::retarget::{Original, Retargeted, retarget};
use super::wrong_effect;
use crate::copies::SpellCopy;
use crate::events::Event;
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each spell the effect names that is still on the stack is copied `count` times, one copy after the other, so the last
/// copy is on top. A copy is not cast: no cast trigger sees it, nothing is paid and no `SpellCast` is logged. It is a
/// token spell with the copiable values of the face the spell is in use as (CR 707.10, 707.8) and the exceptions of the
/// effect, and the modes, X, optional costs and targets of the original. The player `who` names (the controller of the effect by default) controls it and,
/// when the effect lets them, may choose new targets for it (`retarget`). A spell that has left the stack or is an
/// ability is passed over.
///
/// A step with k questions runs k + 1 times; every copy's questions are read from the journal in the order of the copies.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CopySpell {
        who,
        what,
        count,
        new_targets,
        except,
    } = effect
    else {
        return Err(wrong_effect("CopySpell"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let copies = u32::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(u32::MAX);
    let spells: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|spell| spell.zone() == Zone::Stack && spell.kind() != ObjectKind::Ability)
        .collect();
    let rules =
        |spell: &crate::object::GameObject| crate::resolve::spell_ability(state, spell.id());
    let mut answers = cx.journal().iter();
    let mut planned = Vec::new();
    for spell in &spells {
        let ability = rules(spell);
        for _ in 0..copies {
            // A permanent spell has no rules of a spell (an Aura asks for its host through its keyword): its copy
            // announces what it did.
            let choices = if let Some((_, ability)) = &ability {
                let original = Original {
                    id: spell.id(),
                    name: spell.card().name.as_str(),
                    choices: spell.choices(),
                    ability,
                    origin: spell.reference(),
                    cause: None,
                };
                match retarget(cx, player, &mut answers, original, *new_targets)? {
                    Retargeted::Ask(decision) => return Ok(Run::Ask(decision)),
                    Retargeted::Done(choices) => choices,
                }
            } else {
                let mut choices = spell.choices().clone();
                choices.cost_picks = None;
                choices
            };
            planned.push((*spell, choices));
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a copy of a spell that was not asked".into(),
        ));
    }
    let mut events = Vec::with_capacity(planned.len());
    for (id, (spell, mut choices)) in (state.next_object().get()..).zip(planned) {
        let mut definition = spell.card().copiable();
        for modification in except {
            if !definition.except(modification) {
                return Err(ResolveError::Unsupported("Effect.CopySpell.Except"));
            }
        }
        // The definition is the face the spell is in use as, so the copy is not in use as a face of anything.
        choices.face = None;
        events.push(Event::SpellCopied(Box::new(SpellCopy {
            object: ObjectId(id),
            of: spell.reference(),
            controller: player,
            definition: Arc::new(definition),
            choices,
        })));
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
