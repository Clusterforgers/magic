//! A last sentence of a spell that a writer words by itself after the steps the renderers write: "It deals damage equal
//! to its power to each other creature." and "If a creature is dealt damage this way, it gets +5/+0 until end of turn."

use super::super::super::selectors::{Form, phrase_text};
use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Condition, StatKind};

/// Adds the wordings of a spell that ends in one of the sentences, after the wordings of its other steps.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { effects, .. } = ability else {
        return;
    };
    let Some((last, head)) = effects.split_last() else {
        return;
    };
    let Some(tail) = head.last().and_then(|before| sentence(before, last)) else {
        return;
    };
    let mut shorter = ability.clone();
    if let Ability::Spell { effects, .. } = &mut shorter {
        effects.pop();
    }
    texts.extend(
        variants(&shorter)
            .iter()
            .filter(|text| text.ends_with('.'))
            .map(|text| format!("{text} {tail}")),
    );
}

/// The sentence of the last step, given the step before it.
fn sentence(before: &Effect, last: &Effect) -> Option<String> {
    match last {
        Effect::DealDamage {
            source: Some(Ref::Target(0)),
            to: Ref::Each(group),
            amount:
                Amount::Stat {
                    of: Ref::Target(0),
                    stat: StatKind::Power,
                },
        } => {
            let others = Pred::Not(Box::new(Pred::Is(Ref::Target(0))));
            let rest = Selector(
                group
                    .0
                    .iter()
                    .filter(|pred| **pred != others)
                    .cloned()
                    .collect(),
            );
            (rest.0.len() + 1 == group.0.len()).then_some(())?;
            let kind = phrase_text(&rest, Form::Each)?;
            Some(format!(
                "It deals damage equal to its power to each other {kind}."
            ))
        }
        Effect::If {
            condition:
                Condition::Matches {
                    subject: Ref::Target(0),
                    selector,
                },
            then,
            otherwise,
        } if otherwise.is_empty()
            && matches!(
                before,
                Effect::DealDamage {
                    to: Ref::Target(0),
                    ..
                }
            ) =>
        {
            let [Pred::Type(kind)] = selector.0.as_slice() else {
                return None;
            };
            let [
                Effect::Continuous {
                    affected: Ref::Target(0),
                    mods,
                    duration: Duration::UntilEndOfTurn,
                },
            ] = then.as_slice()
            else {
                return None;
            };
            let [
                Modification::ModifyPT {
                    power: Amount::Fixed(power),
                    toughness: Amount::Fixed(toughness),
                },
            ] = mods.as_slice()
            else {
                return None;
            };
            Some(format!(
                "If a {} is dealt damage this way, it gets {}/{} until end of turn.",
                format!("{kind:?}").to_lowercase(),
                signed(*power),
                signed(*toughness)
            ))
        }
        _ => None,
    }
}

/// A change to a characteristic with its sign.
fn signed(number: i32) -> String {
    if number >= 0 {
        format!("+{number}")
    } else {
        number.to_string()
    }
}
