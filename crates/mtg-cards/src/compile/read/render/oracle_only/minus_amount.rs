//! An amount counted and then lowered: "deals X damage to that player, where X is the number of cards in their hand
//! minus 4." The renderers word neither the count of a hand nor the number taken off.

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// Adds the wordings of damage whose amount is the cards in the hand of the player of the event less a number.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [
        Effect::DealDamage {
            source,
            to,
            amount: Amount::Sum(parts),
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let hand = Selector(vec![
        Pred::In(ZoneKind::Hand),
        Pred::OwnedBy(Ref::Triggering),
    ]);
    let [Amount::Count(counted), Amount::Fixed(less @ ..0)] = parts.as_slice() else {
        return;
    };
    if *counted != hand {
        return;
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut plain {
        *effects = vec![Effect::DealDamage {
            source: source.clone(),
            to: to.clone(),
            amount: Amount::X,
        }];
    }
    let tail = format!(
        ", where X is the number of cards in their hand minus {}.",
        less.unsigned_abs()
    );
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text}{tail}"))
        .collect();
    texts.extend(said);
}
