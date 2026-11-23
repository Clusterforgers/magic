//! The one target of a spell and what it could target in its place (CR 115.7, 707.10c): "copy that spell for each other
//! creature that spell could target", "nonland permanents that spell could target", "the copy targets the chosen
//! permanent". A spell on the stack is read as it is, one that has left it as the turn kept it.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_core::ids::PlayerId;

use crate::choose::{Choices, Target, announced_specs, legal_from, spec_bases};
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// The one target of a spell, the places it holds and what is a legal target in each of them.
pub(crate) struct Sole {
    /// The target.
    pub(crate) only: Target,
    /// The indexes of the target specifications it is chosen for.
    pub(crate) places: Vec<usize>,
    /// What could be the target in each of those places, judged for the spell's controller.
    pub(crate) legal: Vec<Target>,
}

impl Sole {
    /// The choices of a copy whose one target is `to` instead.
    pub(crate) fn retargeted(&self, original: &Choices, to: Target) -> Choices {
        let mut choices = original.clone();
        choices.forget_cost_picks();
        for at in &self.places {
            for slot in &mut choices.targets[*at] {
                if *slot == self.only {
                    *slot = to;
                }
            }
        }
        choices
    }
}

/// A spell as it is on the stack, or as it last was there: its reference, rules, announced choices and the player its
/// targets are judged for.
pub(crate) struct SpellSeen<'a, A = Arc<Ability>> {
    /// The spell.
    pub(crate) origin: ObjectRef,
    /// Its spell ability.
    pub(crate) ability: A,
    /// What it announced.
    pub(crate) choices: &'a Choices,
    /// Its controller.
    pub(crate) judge: PlayerId,
}

/// The spell `named` names: on the stack, else as the turn kept it once it left (its controller then taken as `asker`).
pub(crate) fn r114_spell_seen(
    state: &GameState,
    named: ObjectRef,
    asker: PlayerId,
) -> Option<SpellSeen<'_>> {
    if let Some(spell) = state
        .object(named.id)
        .filter(|spell| spell.counter() == named.counter && spell.zone() == Zone::Stack)
    {
        let (_, ability) = crate::resolve::spell_ability(state, spell.id())?;
        return Some(SpellSeen {
            origin: spell.reference(),
            ability,
            choices: spell.choices(),
            judge: crate::characteristics::controller_of(state, spell.id()).unwrap_or(asker),
        });
    }
    let kept = state
        .tallies
        .left_spells
        .iter()
        .rev()
        .find(|kept| kept.of == named)?;
    let ability = crate::ability_reader::printed_rules(&kept.definition)
        .find(|rule| matches!(***rule, Ability::Spell { .. }))?;
    Some(SpellSeen {
        origin: kept.of,
        ability: Arc::clone(ability),
        choices: &kept.choices,
        judge: asker,
    })
}

/// The one target of the spell and what could be the target in its place: none for a spell with no target or with two
/// different ones (the same object in several places is one target).
pub(crate) fn r114_sole_target<A: std::ops::Deref<Target = Ability>>(
    state: &GameState,
    spell: &SpellSeen<'_, A>,
) -> Option<Sole> {
    let original = spell.choices;
    let mut named = original.targets.iter().flatten();
    let only = named.next().copied()?;
    if named.any(|other| *other != only) {
        return None;
    }
    let specs = announced_specs(&spell.ability, &original.modes);
    let bases = spec_bases(&spell.ability, &original.modes);
    let places: Vec<usize> = (0..original.targets.len())
        .filter(|at| original.targets[*at].contains(&only))
        .collect();
    let mut legal_everywhere: Option<Vec<Target>> = None;
    for at in &places {
        let spec = specs.get(*at)?;
        let legal = legal_from(
            state,
            spec,
            spell.origin,
            &[],
            spell.judge,
            None,
            original.x,
            original.modes_chooser(),
            &original.targets[bases.get(*at).copied().unwrap_or(0).min(*at)..*at],
            crate::choose::r118_kind_of(Some(&spell.ability)),
        );
        legal_everywhere = Some(match legal_everywhere {
            None => legal,
            Some(kept) => kept.into_iter().filter(|t| legal.contains(t)).collect(),
        });
    }
    Some(Sole {
        only,
        places,
        legal: legal_everywhere.unwrap_or_default(),
    })
}
