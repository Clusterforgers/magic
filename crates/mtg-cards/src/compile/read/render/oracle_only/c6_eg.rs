//! Wordings of the engine-gap pass (r2bf, r2bg, r2bh): "you may pay that much life" for life of the amount of the
//! event. The renderers word a payment of a fixed amount of life only.

use super::super::variants;
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::effect::Effect;
use crate::value::Amount;

/// A number no card prints, rendered in place of the amount of the event and then replaced by its words.
const C6_EG_MARK: i32 = 7_919;

/// Adds the wordings of the pass.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    c6_eg_that_much_life(ability, texts);
}

/// "You may pay that much life. If you do, draw that many cards.": the wordings of a payment of a fixed amount of life
/// with the amount of the event in its place.
fn c6_eg_that_much_life(ability: &Ability, texts: &mut Vec<String>) {
    let mut fixed = ability.clone();
    let mut found = false;
    if let Ability::Triggered { effects, .. } = &mut fixed {
        for effect in effects.iter_mut() {
            let Effect::Pay { cost, .. } = effect else {
                continue;
            };
            for item in &mut cost.0 {
                if *item == CostItem::PayLife(Amount::Triggering) {
                    *item = CostItem::PayLife(Amount::Fixed(C6_EG_MARK));
                    found = true;
                }
            }
        }
    }
    if !found {
        return;
    }
    let mark = format!("pay {C6_EG_MARK} life");
    let said: Vec<String> = variants(&fixed)
        .into_iter()
        .filter(|text| text.contains(&mark))
        .map(|text| text.replace(&mark, "pay that much life"))
        .collect();
    texts.extend(said);
}
