//! A return of a group a target player controls: "Return all nonland permanents target player controls to their
//! owner's hand."

use super::super::super::selectors::{Form, phrase_text};
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Targetable};

/// Adds the wording of a spell that returns every permanent of a kind that its one target player controls.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let [spec] = targets.as_slice() else {
        return;
    };
    let (
        Targetable::Player | Targetable::Opponent,
        [
            Effect::Move {
                what: Ref::Each(group),
                to: Destination::Hand,
            },
        ],
    ) = (&spec.what, effects.as_slice())
    else {
        return;
    };
    if !group.0.contains(&Pred::ControlledBy(Ref::Target(0))) {
        return;
    }
    let Some(phrase) = phrase_text(group, Form::All) else {
        return;
    };
    let sentence = format!("return all {phrase} to their owner's hand");
    texts.extend(with_effects(ability, &sentence));
}
