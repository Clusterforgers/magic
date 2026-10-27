//! A return that goes to the battlefield instead after more life gained: "At the beginning of your end step, if you
//! gained life this turn, return target creature card from your graveyard to your hand. If you gained 7 or more life
//! this turn, return that card to the battlefield instead." The renderers write the plain return, which is the twin
//! with the `otherwise` steps and no intervening "if".

use super::c5_af_variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::value::{Amount, Cmp, Condition, TurnTally};

/// The life the controller gained this turn, read through a tally.
fn c5_af_gained(amount: &Amount, what: TurnTally) -> bool {
    matches!(amount, Amount::Tallied { who, what: tally } if who.is_you() && *tally == what)
}

/// Whether the condition is "if you gained life this turn".
fn c5_af_gained_life(condition: &Condition) -> bool {
    matches!(
        condition,
        Condition::Compare { left, op: Cmp::Gt, right: Amount::Fixed(0) }
            if c5_af_gained(left, TurnTally::LifeGained)
    )
}

/// The life the condition "if you gained N or more life this turn" asks for.
fn c5_af_gained_at_least(condition: &Condition) -> Option<i32> {
    match condition {
        Condition::Compare {
            left,
            op: Cmp::Ge,
            right: Amount::Fixed(life),
        } if c5_af_gained(left, TurnTally::LifeGainedTotal) => Some(*life),
        _ => None,
    }
}

/// Whether the steps return the first target to the battlefield.
fn c5_af_to_battlefield(then: &[Effect]) -> bool {
    let rules = serde_json::to_string(then).unwrap_or_default();
    rules == r#"[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]"#
}

/// Adds the wordings of a triggered return to the hand that goes to the battlefield instead after more life gained.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if: Some(first),
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::If {
            condition,
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let Some(life) = c5_af_gained_at_least(condition) else {
        return;
    };
    if !c5_af_gained_life(first) || !c5_af_to_battlefield(then) || otherwise.is_empty() {
        return;
    }
    let mut plain = ability.clone();
    if let Ability::Triggered {
        intervening_if,
        effects,
        ..
    } = &mut plain
    {
        *intervening_if = None;
        effects.clone_from(otherwise);
    }
    let instead = format!(
        " If you gained {life} or more life this turn, return that card to the battlefield instead."
    );
    texts.extend(c5_af_variants(&plain).iter().filter_map(|text| {
        let (head, rest) = text.split_once(", ")?;
        Some(format!(
            "{head}, if you gained life this turn, {rest}{instead}"
        ))
    }));
}
