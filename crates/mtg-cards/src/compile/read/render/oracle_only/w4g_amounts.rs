//! Printed wordings of an amount that the renderers write another way: "Add an amount of {C} equal to the number of
//! time counters on ~" for "Add {C} for each time counter on ~", and "Double all damage that creatures you control with
//! counters on them would deal" for the replacement that doubles the damage those sources would deal.

use super::super::super::amounts::counted_parts;
use super::super::super::selectors::{Form, phrase_text};
use super::super::equal_text;
use crate::ability::Ability;
use crate::effect::{Effect, Produce};
use crate::refs::Ref;
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// Adds to `texts` the printed wordings of the ability's amount.
pub(super) fn w4g_add_amounts(ability: &Ability, texts: &mut Vec<String>) {
    w4g_mana_amount(ability, texts);
    w4g_double_all(ability, texts);
}

/// "Add {C} for each time counter on ~" is also "Add an amount of {C} equal to the number of time counters on ~".
fn w4g_mana_amount(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::AddMana {
            who: Ref::You,
            mana: Produce::Fixed(list),
            times,
            ..
        },
    ] = ability.effects()
    else {
        return;
    };
    let ([one], Some((Amount::Fixed(1), tail)), Some(equal)) =
        (list.as_slice(), counted_parts(times), equal_text(times))
    else {
        return;
    };
    let counted = format!("Add {{{}}} {tail}", one.symbol());
    let amount = format!("Add an amount of {{{}}} equal to {equal}", one.symbol());
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(&counted))
        .map(|text| text.replace(&counted, &amount))
        .collect();
    texts.extend(found);
}

/// "Double all damage that <sources> would deal.": the replacement that doubles the damage of a group of sources to
/// any recipient.
fn w4g_double_all(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when:
            ReplaceWhen::Damage {
                to: None,
                source,
                combat,
            },
        by: ReplaceBy::Multiply(2),
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    if source.0.is_empty() {
        return;
    }
    let Some(sources) = phrase_text(source, Form::All) else {
        return;
    };
    let kind = match combat {
        None => "",
        Some(true) => "combat ",
        Some(false) => return,
    };
    texts.push(format!("Double all {kind}damage that {sources} would deal."));
}
