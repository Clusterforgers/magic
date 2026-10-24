//! A payment of mana and life: "you may pay {1} and 1 life. If you do, draw a card." The renderers word a payment of one
//! part.

use super::super::variants;
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::effect::Effect;
use crate::value::Amount;

/// Adds the wordings of a payment of mana and a fixed amount of life, from the wordings of the payment of the mana.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut mana_only = ability.clone();
    let mut paid = Vec::new();
    if let Ability::Triggered { effects, .. }
    | Ability::Activated { effects, .. }
    | Ability::Spell { effects, .. } = &mut mana_only
    {
        for effect in effects.iter_mut() {
            let Effect::Pay { cost, .. } = effect else {
                continue;
            };
            if let [CostItem::Mana(mana), CostItem::PayLife(Amount::Fixed(life))] =
                cost.0.as_slice()
            {
                paid.push((
                    format!("pay {mana}."),
                    format!("pay {mana} and {life} life."),
                ));
                cost.0 = vec![CostItem::Mana(mana.clone())];
            }
        }
    }
    for (from, to) in paid {
        texts.extend(
            variants(&mana_only)
                .iter()
                .filter(|text| text.contains(&from))
                .map(|text| text.replacen(&from, &to, 1)),
        );
    }
}
