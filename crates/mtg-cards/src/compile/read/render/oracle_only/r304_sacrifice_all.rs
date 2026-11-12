//! A sacrifice of every permanent of a kind the player controls: "sacrifice all lands you control" (the count is the
//! number of those permanents). Written from the sacrifice of two of the kind, with "two" and the plural replaced.

use super::super::super::selectors::{Form, phrase_text};
use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;

/// Adds "sacrifice all <kind> you control" for a sacrifice whose count is the number of the permanents it sacrifices.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let Some(at) = effects.iter().position(all_controlled) else {
        return;
    };
    let Effect::Sacrifice { filter, .. } = &effects[at] else {
        return;
    };
    let kind = Selector(
        filter
            .0
            .iter()
            .filter(|pred| **pred != Pred::ControlledBy(Ref::You))
            .cloned()
            .collect(),
    );
    let (Some(plural), Some(all)) = (
        phrase_text(&kind, Form::All),
        phrase_text(filter, Form::All),
    ) else {
        return;
    };
    let mut stand_in = effects.to_vec();
    stand_in[at] = Effect::Sacrifice {
        who: Ref::You,
        filter: kind,
        count: Amount::Fixed(2),
        bind: None,
    };
    let Some(bare) = with_effects(ability, stand_in) else {
        return;
    };
    let said = format!("acrifice two {plural}");
    let meant = format!("acrifice all {all}");
    for text in variants(&bare) {
        if text.contains(&said) {
            let text = text.replacen(&said, &meant, 1);
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
}

/// A sacrifice by the controller of every permanent its filter names, among those it controls.
fn all_controlled(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::Sacrifice { who: Ref::You, filter, count: Amount::Count(counted), bind: None }
            if counted == filter && filter.0.contains(&Pred::ControlledBy(Ref::You))
    )
}
