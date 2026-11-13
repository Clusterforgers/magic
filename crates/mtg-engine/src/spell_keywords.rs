//! Keywords the static abilities of permanents give to the spells a player casts ("Nonartifact spells you cast have
//! improvise", "Artifact spells you cast have convoke"): the keywords a spell has for the help that pays for it
//! (`help::offered`).
//!
//! A spell is not a permanent, so the layers do not hold it; the static abilities that name a spell are read from the
//! permanents as the layers leave them, with the spell judged as the announcement judges it, on the stack under its
//! caster though it is still in a hand (CR 601.2a).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Modification, SpellBenefit};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::ability_reader::{abilities_of, definition_has_static};
use crate::characteristics::controller_of_object;
use crate::object::{GameObject, ObjectKind};
use crate::resolve::Scope;
use crate::rules::Rule;
use crate::state::GameState;
use crate::zones::Zone;

/// The keywords that static abilities of the permanents in play give to `spell`, which `caster` casts, each once, in
/// the order they were found.
pub(crate) fn given_to_spell(state: &GameState, caster: PlayerId, spell: ObjectId) -> Vec<Keyword> {
    let mut given: Vec<Keyword> = Vec::new();
    let Some(target) = state.object(spell).map(GameObject::reference) else {
        return given;
    };
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state
            .object(id)
            .filter(|object| object.kind() != ObjectKind::Ability)
            .filter(|object| definition_has_static(object.card()))
        else {
            continue;
        };
        let controller = controller_of_object(state, object);
        if !state.is_alive(controller) {
            continue;
        }
        for (_, ability) in abilities_of(state, id) {
            let Some((selector, mods)) = naming_spells(&ability) else {
                continue;
            };
            let Ability::Static { condition, .. } = &*ability else {
                continue;
            };
            let scope = Scope {
                casting: Some((spell, caster)),
                ..Scope::announcing(state, controller, object.reference())
            };
            let applies = scope.selects(selector, target).unwrap_or(false)
                && condition
                    .as_ref()
                    .is_none_or(|condition| scope.holds(condition).unwrap_or(false));
            if !applies {
                continue;
            }
            for modification in mods {
                if let Modification::AddKeyword(keyword) = modification {
                    let keyword = r5510_read_own_cost(state, spell, keyword);
                    if !given.contains(&keyword) {
                        given.push(keyword);
                    }
                }
            }
        }
    }
    for effect in &state.rule_effects {
        let Rule::NextSpell {
            what,
            benefit: SpellBenefit::Keyword(keyword),
        } = &effect.rule
        else {
            continue;
        };
        let scope = Scope {
            casting: Some((spell, caster)),
            ..Scope::announcing(state, effect.controller, effect.source.unwrap_or(target))
        };
        if effect.controller == caster
            && (what.is_empty() || scope.selects(what, target).unwrap_or(false))
            && !given.contains(keyword)
        {
            given.push(keyword.clone());
        }
    }
    given
}

/// The selector and changes of a static ability of the battlefield that names spells.
fn naming_spells(ability: &Arc<Ability>) -> Option<(&Selector, &[Modification])> {
    let Ability::Static {
        affected: Ref::Each(selector),
        mods,
        defining: false,
        from,
        ..
    } = &**ability
    else {
        return None;
    };
    (from.is_battlefield()
        && (selector.0.contains(&Pred::Spell) || selector.0.contains(&Pred::In(ZoneKind::Stack))))
    .then_some((selector, mods.as_slice()))
}

/// The keyword with a cost read off the spell's card ("The replicate cost is equal to its mana cost"), as the layers
/// read it once the spell is on the stack.
fn r5510_read_own_cost(state: &GameState, spell: ObjectId, keyword: &Keyword) -> Keyword {
    let Some(object) = state.object(spell) else {
        return keyword.clone();
    };
    let own = object.mana_cost().cloned().unwrap_or_default();
    keyword
        .clone()
        .with_own_mana_cost_x(&own, object.card().x_in_cost)
}
