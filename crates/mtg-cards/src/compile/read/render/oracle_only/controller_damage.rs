//! Damage to the controller of the target: "~ deals 3 damage to that creature's controller." The renderers word the
//! controller of a target only as the subject of a step ("its controller").

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Targetable};
use mtg_core::card::CardType;

/// The effects with the damage to the controller of the target made damage to the player of an event, and whether any was.
fn swapped(effects: &[Effect]) -> (Vec<Effect>, bool) {
    let mut found = false;
    let mut out = Vec::new();
    for effect in effects {
        out.push(match effect {
            Effect::DealDamage { source, to, amount }
                if *to == Ref::ControllerOf(Box::new(Ref::Target(0))) =>
            {
                found = true;
                Effect::DealDamage {
                    source: source.clone(),
                    to: Ref::Triggering,
                    amount: amount.clone(),
                }
            }
            Effect::If {
                condition,
                then,
                otherwise,
            } => {
                let ((then, a), (otherwise, b)) = (swapped(then), swapped(otherwise));
                found |= a || b;
                Effect::If {
                    condition: condition.clone(),
                    then,
                    otherwise,
                }
            }
            other => other.clone(),
        });
    }
    (out, found)
}

/// Adds the wordings of damage to the controller of a creature or permanent target.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let Some(Targetable::Object(selector)) = targets.first().map(|spec| &spec.what) else {
        return;
    };
    let (effects, found) = swapped(effects);
    if !found {
        return;
    }
    let noun = if selector.0.contains(&Pred::Type(CardType::Creature)) {
        "creature"
    } else {
        "permanent"
    };
    let mut plain = ability.clone();
    if let Ability::Spell { effects: steps, .. } = &mut plain {
        *steps = effects;
    }
    let (from, into) = (
        " damage to that player",
        format!(" damage to that {noun}'s controller"),
    );
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, &into, 1))
        .collect();
    texts.extend(said);
}
