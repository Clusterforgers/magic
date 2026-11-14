//! `Effect::CopySpell`: copies of spells on the stack are put on the stack above them (CR 707.10).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::modify::Modification;
use mtg_cards::refs::Ref;
use mtg_core::ids::{ObjectId, PlayerId};

use super::retarget::{Original, Retargeted, retarget};
use super::wrong_effect;
use crate::copies::{KeptSpell, SpellCopy};
use crate::events::Event;
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each spell the effect names that is still on the stack is copied `count` times, one copy after the other, so the last
/// copy is on top. A copy is not cast: no cast trigger sees it, nothing is paid and no `SpellCast` is logged. It is a
/// token spell with the copiable values of the face the spell is in use as (CR 707.10, 707.8) and the exceptions of the
/// effect, and the modes, X, optional costs and targets of the original. The player `who` names (the controller of the effect by default) controls it and,
/// when the effect lets them, may choose new targets for it (`retarget`). A spell that has left the stack or is an
/// ability is passed over, except the source spell of a delayed ability that kept it ("copy this spell", epic): the copy
/// is made from the spell as it resolved (`copy_kept`).
///
/// A step with k questions runs k + 1 times; every copy's questions are read from the journal in the order of the copies.
/// A replacement that adds copies (`ReplaceWhen::CopiesSpell`) adds them after the others, each with new targets.
#[allow(clippy::too_many_lines)]
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CopySpell {
        who,
        what,
        count,
        new_targets,
        except,
        each_target,
        each_player,
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
    // A copy of a spell one or more times may be replaced by more copies, each with new targets (Twinning Staff).
    let added = if copies > 0 || each_target.is_some() {
        crate::replace::r5516_copies::r5516_added_copies(state, player)?
    } else {
        0
    };
    let spells: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|spell| spell.zone() == Zone::Stack && spell.kind() != ObjectKind::Ability)
        .filter(|spell| !r302_uncopyable(spell.card()))
        .collect();
    if spells.is_empty()
        && let Some(kept) = left_spell(cx, what)?
    {
        return copy_kept(cx, player, kept, (copies, added), *new_targets, except);
    }
    let rules =
        |spell: &crate::object::GameObject| crate::resolve::spell_ability(state, spell.id());
    let mut answers = cx.journal().iter();
    let mut planned = Vec::new();
    for spell in &spells {
        let ability = rules(spell);
        let mut made = copies;
        if let Some(selector) = each_target {
            let Some((_, ability)) = &ability else {
                continue;
            };
            let choices =
                each_target_choices(cx, (player, *each_player), spell, ability, selector)?;
            made = u32::try_from(choices.len()).unwrap_or(u32::MAX);
            planned.extend(choices.into_iter().map(|choices| (*spell, choices)));
            if made == 0 {
                continue;
            }
        }
        let start = if each_target.is_some() { made } else { 0 };
        let total = if made > 0 {
            made.saturating_add(added)
        } else {
            0
        };
        for n in start..total {
            // An added copy may always get new targets.
            let new_targets = *new_targets || n >= made;
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
                match retarget(cx, player, &mut answers, original, new_targets)? {
                    Retargeted::Ask(decision) => return Ok(Run::Ask(decision)),
                    Retargeted::Done(choices) => choices,
                }
            } else {
                let mut choices = spell.choices().clone();
                choices.forget_cost_picks();
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
            left: false,
        })));
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The spell the resolving delayed ability kept as it was set up, when its source spell has left the stack (epic).
fn kept_spell<'a>(cx: &'a Cx<'_>) -> Option<&'a KeptSpell> {
    let state = cx.state();
    let id = state.object(cx.resolving.object())?.ability()?.delayed()?;
    state
        .delayed()
        .iter()
        .find(|waiting| waiting.id() == id)?
        .kept()
}

/// The spell `what` names as it last was on the stack, when it has left it: the one a delayed ability kept of its source
/// ("copy this spell", epic), else the one the turn kept ("copy that spell" after another effect returned it to its
/// owner's hand). The copy is made by last known information (CR 608.2h, 707.10).
fn left_spell<'a>(cx: &'a Cx<'_>, what: &Ref) -> Result<Option<&'a KeptSpell>, ResolveError> {
    if *what == Ref::This
        && let Some(kept) = kept_spell(cx)
    {
        return Ok(Some(kept));
    }
    let left = &cx.state().tallies.left_spells;
    Ok(cx
        .scope
        .objects(what)?
        .into_iter()
        .find_map(|named| left.iter().rev().find(|kept| kept.of == named)))
}

/// Whether the spell says "This spell can't be copied." (CR 707.10): no effect makes a copy of it.
fn r302_uncopyable(definition: &mtg_cards::CardDefinition) -> bool {
    crate::ability_reader::printed_rules(definition).any(|rule| {
        matches!(
            &**rule,
            Ability::Rule {
                rule: mtg_cards::modify::GameRule::CantBeCopied,
                ..
            }
        )
    })
}

/// Copies of the spell a delayed ability kept, `copies` of them and `added` more with new targets, made the way copies of a spell on the stack are: with the
/// exceptions of the effect, and the modes, X and targets of the original, which the controller may change when the effect
/// lets them. The copy goes on top of the stack (CR 702.50a, 707.10).
fn copy_kept(
    cx: &Cx<'_>,
    player: PlayerId,
    kept: &KeptSpell,
    (copies, added): (u32, u32),
    new_targets: bool,
    except: &[Modification],
) -> Result<Run, ResolveError> {
    let state = cx.state();
    if r302_uncopyable(&kept.definition) {
        return Ok(Run::events(Vec::new()));
    }
    let mut definition = (*kept.definition).clone();
    for modification in except {
        if !definition.except(modification) {
            return Err(ResolveError::Unsupported("Effect.CopySpell.Except"));
        }
    }
    let ability = crate::ability_reader::printed_rules(&kept.definition)
        .find(|rule| matches!(***rule, Ability::Spell { .. }));
    let mut answers = cx.journal().iter();
    let mut planned = Vec::new();
    let total = if copies > 0 {
        copies.saturating_add(added)
    } else {
        0
    };
    for n in 0..total {
        let choices = if let Some(ability) = ability {
            let original = Original {
                id: kept.of.id,
                name: kept.definition.name.as_str(),
                choices: &kept.choices,
                ability,
                origin: kept.of,
                cause: None,
            };
            match retarget(
                cx,
                player,
                &mut answers,
                original,
                new_targets || n >= copies,
            )? {
                Retargeted::Ask(decision) => return Ok(Run::Ask(decision)),
                Retargeted::Done(choices) => choices,
            }
        } else {
            kept.choices.clone()
        };
        planned.push(choices);
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a copy of a spell that was not asked".into(),
        ));
    }
    let definition = Arc::new(definition);
    let events = (state.next_object().get()..)
        .zip(planned)
        .map(|(id, choices)| {
            Event::SpellCopied(Box::new(SpellCopy {
                object: ObjectId(id),
                of: kept.of,
                controller: player,
                definition: Arc::clone(&definition),
                choices,
                left: true,
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

/// The choices of the copies of `spell` "for each other object `selector` names that the spell could target" (CR 707.10c):
/// a spell whose targets are all one object (once, or in several places: "if all of those targets are" that creature)
/// gets one copy for each such other object that is a legal target in each of those places, and each copy targets a
/// different one there; each other player it could target gets one too when `players` is set. What the spell could target
/// is judged for the spell's controller, whoever controls the copies.
/// A spell with no target or with two different ones gets none.
fn each_target_choices(
    cx: &Cx<'_>,
    (player, players): (mtg_core::ids::PlayerId, bool),
    spell: &crate::object::GameObject,
    ability: &mtg_cards::ability::Ability,
    selector: &mtg_cards::refs::Selector,
) -> Result<Vec<crate::choose::Choices>, ResolveError> {
    use crate::choose::{Target, announced_specs, legal_from, spec_bases};
    let state = cx.state();
    let original = spell.choices();
    let mut named = original.targets.iter().flatten();
    let Some(only) = named.next().copied() else {
        return Ok(Vec::new());
    };
    if named.any(|other| *other != only) {
        return Ok(Vec::new());
    }
    let judge = crate::characteristics::controller_of(state, spell.id()).unwrap_or(player);
    let specs = announced_specs(ability, &original.modes);
    let bases = spec_bases(ability, &original.modes);
    let places: Vec<usize> = (0..original.targets.len())
        .filter(|at| original.targets[*at].contains(&only))
        .collect();
    let mut legal_everywhere: Option<Vec<Target>> = None;
    for at in &places {
        let Some(spec) = specs.get(*at) else {
            return Ok(Vec::new());
        };
        let legal = legal_from(
            state,
            spec,
            spell.reference(),
            &[],
            judge,
            None,
            original.x,
            original.modes_chooser(),
            &original.targets[bases.get(*at).copied().unwrap_or(0).min(*at)..*at],
        );
        legal_everywhere = Some(match legal_everywhere {
            None => legal,
            Some(kept) => kept.into_iter().filter(|t| legal.contains(t)).collect(),
        });
    }
    let mut found = Vec::new();
    for target in legal_everywhere.unwrap_or_default() {
        let wanted = match target {
            Target::Object(object) => cx.scope.selects(selector, object)?,
            Target::Player(_) => players,
        };
        if target == only || !wanted {
            continue;
        }
        let mut choices = original.clone();
        choices.forget_cost_picks();
        for at in &places {
            for slot in &mut choices.targets[*at] {
                if *slot == only {
                    *slot = target;
                }
            }
        }
        found.push(choices);
    }
    Ok(found)
}
