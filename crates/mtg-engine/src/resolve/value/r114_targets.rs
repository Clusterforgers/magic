//! The targets of a spell as the predicates read them: on the stack, or as the spell last was there once it has left
//! (CR 608.2h: "copy that spell" after the spell was countered), and what the spell could target in place of its one
//! target (CR 707.10c).

use mtg_cards::refs::{Ref, Selector};

use super::{ResolveError, Scope, Seen};
use crate::choose::{Choices, Target};
use crate::object::ObjectRef;
use crate::zones::Zone;

impl<'a> Scope<'a> {
    /// What the spell or ability announced: as it is on the stack, or as the turn kept a spell that has left it.
    pub(super) fn r114_stack_choices(
        &self,
        reference: ObjectRef,
        seen: &Seen<'a>,
    ) -> Option<&'a Choices> {
        if seen.zone == Zone::Stack
            && let Some(card) = seen.card.filter(|card| card.zone() == Zone::Stack)
        {
            return Some(card.choices());
        }
        self.state
            .tallies
            .left_spells
            .iter()
            .rev()
            .find(|kept| kept.of == reference)
            .map(|kept| &kept.choices)
    }

    /// Whether every target of the spell is one object the selector names, in one place or several.
    pub(super) fn r114_targets_only_one(
        &self,
        inner: &Selector,
        reference: ObjectRef,
        seen: &Seen<'a>,
    ) -> Result<bool, ResolveError> {
        let Some(choices) = self.r114_stack_choices(reference, seen) else {
            return Ok(false);
        };
        let mut targets = choices.targets.iter().flatten();
        let Some(Target::Object(only)) = targets.next().copied() else {
            return Ok(false);
        };
        if targets.any(|other| *other != Target::Object(only)) {
            return Ok(false);
        }
        self.selects(inner, only)
    }

    /// Whether the object could be the target of one of the spells `named` names in place of its one target.
    pub(super) fn r114_could_target(
        &self,
        named: &Ref,
        reference: ObjectRef,
    ) -> Result<bool, ResolveError> {
        for spell in self.objects(named)? {
            let Some(seen) =
                crate::r114_sole_target::r114_spell_seen(self.state, spell, self.controller)
            else {
                continue;
            };
            let Some(sole) = crate::r114_sole_target::r114_sole_target(self.state, &seen) else {
                continue;
            };
            if sole.legal.contains(&Target::Object(reference)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The object the source chose as it entered, as the object it was then, wherever it is now.
    pub(super) fn r114_chosen_object(&self) -> Vec<Target> {
        self.state
            .memory
            .get(&(
                self.source,
                crate::replace::r114_choose_object::r114_chosen_slot(),
            ))
            .into_iter()
            .flatten()
            .filter_map(|held| match held {
                crate::memory::Held::Object(object) => Some(Target::Object(*object)),
                _ => None,
            })
            .collect()
    }
}
