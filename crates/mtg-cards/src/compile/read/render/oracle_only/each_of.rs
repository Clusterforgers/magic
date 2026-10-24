//! Damage to each of several targets: "~ deals X damage to each of up to three targets." The renderers word a target
//! specification of one target only; the damage is written for it and the recipient is worded for the bounds.

use super::super::super::amounts::count_word;
use super::super::{Form, phrase_text, spec_text, variants};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Bounds, Ref, TargetSpec, Targetable};
use crate::value::Amount;

/// Adds the wordings of damage to "each of up to N targets" for the first target specification of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(TargetSpec {
        what,
        bounds,
        divide: None,
    }) = ability.targets().first()
    else {
        return;
    };
    let (Some(0), Some(Amount::Fixed(most))) = (bounds.fixed_min(), &bounds.max) else {
        return;
    };
    let Some(most) = u32::try_from(*most).ok().filter(|most| *most >= 2) else {
        return;
    };
    let hits = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::DealDamage {
                source: None,
                to: Ref::Target(0),
                ..
            }
        )
    });
    let (Some(count), true) = (count_word(most), hits) else {
        return;
    };
    let mut single = ability.clone();
    let Some(spec) = targets_mut(&mut single).and_then(|targets| targets.first_mut()) else {
        return;
    };
    spec.bounds = Bounds::default();
    let (Some(one), many) = (
        spec_text(spec),
        match what {
            Targetable::AnyTarget => Some(format!("each of up to {count} targets")),
            Targetable::Object(selector) => phrase_text(selector, Form::Targets)
                .map(|group| format!("each of up to {count} target {group}")),
            _ => None,
        },
    ) else {
        return;
    };
    let Some(many) = many else {
        return;
    };
    let (from, into) = (format!(" damage to {one}"), format!(" damage to {many}"));
    texts.extend(
        variants(&single)
            .iter()
            .filter(|text| text.contains(&from))
            .map(|text| text.replacen(&from, &into, 1)),
    );
}

/// The target specifications of an ability that has them.
fn targets_mut(ability: &mut Ability) -> Option<&mut Vec<TargetSpec>> {
    match ability {
        Ability::Spell { targets, .. }
        | Ability::Activated { targets, .. }
        | Ability::Triggered { targets, .. } => Some(targets),
        _ => None,
    }
}
