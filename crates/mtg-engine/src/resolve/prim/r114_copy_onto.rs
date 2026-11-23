//! `Effect::R114CopySpellOnto`: one copy of a spell with one target, targeting a player or object the effect names that
//! the spell could target (CR 707.10c): "choose one of those permanents. Copy that spell. The copy targets the chosen
//! permanent" (Exterminator Magmarch), "choose that player or a permanent they control, copy that spell, and the copy
//! targets the chosen player or permanent" (Zevlor, Elturel Exile).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::retarget::{Original, Retargeted, retarget};
use super::wrong_effect;
use crate::choose::{Choice, Target, TargetOptions};
use crate::copies::SpellCopy;
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::r114_sole_target::{r114_sole_target, r114_spell_seen};
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// Each spell `what` names, on the stack or as it last was there, whose targets are all one player or object, gets one
/// copy controlled by the controller of the effect, whose target is the one the controller picks among the players and
/// objects `onto` names that the spell could target in that target's places; none is made when none could be. The copy is
/// put on the stack and not cast, with the modes, X and costs paid of the original. A replacement that adds copies
/// (`ReplaceWhen::CopiesSpell`) adds them after it, each with new targets.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R114CopySpellOnto { what, onto } = effect else {
        return Err(wrong_effect("R114CopySpellOnto"));
    };
    let state = cx.state();
    let player = cx.controller();
    let mut candidates: Vec<Target> = cx
        .scope
        .players(onto)
        .unwrap_or_default()
        .into_iter()
        .map(Target::Player)
        .collect();
    candidates.extend(
        cx.scope
            .objects(onto)
            .unwrap_or_default()
            .into_iter()
            .map(Target::Object),
    );
    let mut answers = cx.journal().iter();
    let mut planned = Vec::new();
    for named in cx.scope.objects(what)? {
        let Some(seen) = r114_spell_seen(state, named, player) else {
            continue;
        };
        let Some((definition, left)) = r114_definition(state, named) else {
            continue;
        };
        if super::copy_spell::r302_uncopyable(&definition) {
            continue;
        }
        let Some(sole) = r114_sole_target(state, &seen) else {
            continue;
        };
        let legal: Vec<Target> = candidates
            .iter()
            .filter(|target| sole.legal.contains(target) && **target != sole.only)
            .copied()
            .collect();
        let to = match r114_pick(&mut answers, player, named.id, legal)? {
            Picked::Nothing => continue,
            Picked::Ask(decision) => return Ok(Run::Ask(decision)),
            Picked::Target(to) => to,
        };
        let mut choices = sole.retargeted(seen.choices, to);
        choices.face = None;
        let definition = Arc::new(definition);
        planned.push((named, Arc::clone(&definition), choices, left));
        // A copy of a spell may be replaced by more copies, each with new targets (Twinning Staff).
        let added = crate::replace::r5516_copies::r5516_added_copies(state, player)?;
        for _ in 0..added {
            let original = Original {
                id: named.id,
                name: definition.name.as_str(),
                choices: seen.choices,
                ability: &seen.ability,
                origin: seen.origin,
                cause: None,
            };
            let mut choices = match retarget(cx, player, &mut answers, original, true)? {
                Retargeted::Ask(decision) => return Ok(Run::Ask(decision)),
                Retargeted::Done(choices) => choices,
            };
            choices.face = None;
            planned.push((named, Arc::clone(&definition), choices, left));
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a copy of a spell that was not asked".into(),
        ));
    }
    let events = (state.next_object().get()..)
        .zip(planned)
        .map(|(id, (of, definition, choices, left))| {
            Event::SpellCopied(Box::new(SpellCopy {
                object: ObjectId(id),
                of,
                controller: player,
                definition,
                choices,
                left,
            }))
        })
        .collect();
    Ok(Run::events(events))
}

/// The copiable values of the spell `named` names, and whether it has left the stack: as it is on the stack (the face
/// it is in use as), else as the turn kept it.
fn r114_definition(state: &GameState, named: ObjectRef) -> Option<(CardDefinition, bool)> {
    if let Some(spell) = state
        .object(named.id)
        .filter(|spell| spell.counter() == named.counter && spell.zone() == Zone::Stack)
    {
        return Some((spell.card().copiable(), false));
    }
    let kept = state
        .tallies
        .left_spells
        .iter()
        .rev()
        .find(|kept| kept.of == named)?;
    Some(((*kept.definition).clone(), true))
}

/// What the copy's target is.
enum Picked {
    /// The spell could target none of them: no copy.
    Nothing,
    /// The question among several.
    Ask(Decision),
    /// The target.
    Target(Target),
}

/// The target `player` picks for the copy of the spell `source` among `legal`: asked when there are several.
fn r114_pick(
    answers: &mut std::slice::Iter<'_, (PlayerId, Choice)>,
    player: PlayerId,
    source: ObjectId,
    legal: Vec<Target>,
) -> Result<Picked, ResolveError> {
    match legal.as_slice() {
        [] => return Ok(Picked::Nothing),
        [one] => return Ok(Picked::Target(*one)),
        _ => {}
    }
    match answers.next() {
        None => Ok(Picked::Ask(Decision {
            player,
            kind: DecisionKind::ChooseTargets(TargetOptions {
                source,
                spec: 0,
                legal,
                min: 1,
                max: 1,
                can_cancel: false,
                wording: "the target of the copy".to_owned(),
                triggered: false,
                apart: Vec::new(),
                together: None,
                total: None,
                paid: Box::default(),
            }),
        })),
        Some((who, Choice::Targets(picked))) if *who == player => match picked.as_slice() {
            [one] if legal.contains(one) => Ok(Picked::Target(*one)),
            _ => Err(ResolveError::Invalid(format!(
                "{picked:?} is no answer to the target of a copy"
            ))),
        },
        Some(other) => Err(ResolveError::Invalid(format!(
            "{other:?} is no answer to the target of a copy"
        ))),
    }
}
