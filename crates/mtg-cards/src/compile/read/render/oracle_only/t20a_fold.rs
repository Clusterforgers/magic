//! A trigger whose intervening "if" only describes the object of its event is worded as the oracle does, with the
//! description in the event: "Whenever you cast a noncreature spell with mana value 3 or greater" and "Whenever an
//! enchantment you control is put into a graveyard from the battlefield" for a plain cast or move and an "if" about the
//! spell or the card that moved (CR 603.4 judges it against the same state as the event, so both mean the same).

use super::super::variants;
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Condition};
use mtg_core::card::CardType;

/// Adds the wordings of a trigger whose condition describes the object of the event, and of a permanent that is no creature put into a graveyard.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    fold(ability, texts);
    put_into_graveyard(ability, texts);
}

/// Adds the wordings of a trigger whose condition describes the object of the event, as the event words it.
fn fold(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if: Some(held),
        limit: None,
        ..
    } = ability
    else {
        return;
    };
    let Some(preds) = described(held) else {
        return;
    };
    if !matches!(
        when,
        TriggerPattern::ZoneChange { .. } | TriggerPattern::SpellCast { .. }
    ) {
        return;
    }
    for first in [true, false] {
        let mut twin = ability.clone();
        if let Ability::Triggered {
            when: TriggerPattern::ZoneChange { what, .. } | TriggerPattern::SpellCast { what, .. },
            intervening_if,
            ..
        } = &mut twin
        {
            let mut all = if first { what.0.clone() } else { preds.clone() };
            all.extend(if first { preds.clone() } else { what.0.clone() });
            *what = Selector(all);
            *intervening_if = None;
        }
        texts.extend(variants(&twin));
    }
}

/// The predicates a condition about the triggering object puts on that object, when it says nothing else.
fn described(held: &Condition) -> Option<Vec<Pred>> {
    match held {
        Condition::Matches {
            subject: Ref::Triggering,
            selector,
        } if plain(selector) => Some(selector.0.clone()),
        Condition::Compare {
            left:
                Amount::Stat {
                    of: Ref::Triggering,
                    stat,
                },
            op,
            right,
        } if !matches!(right, Amount::Stat { .. }) => Some(vec![Pred::Compare {
            stat: *stat,
            op: *op,
            to: right.clone(),
        }]),
        Condition::Not(inner) => match inner.as_ref() {
            Condition::Matches {
                subject: Ref::Triggering,
                selector,
            } if plain(selector) && selector.0.len() == 1 => {
                Some(vec![Pred::Not(Box::new(selector.0[0].clone()))])
            }
            _ => None,
        },
        Condition::All(parts) if !parts.is_empty() => {
            let mut preds = Vec::new();
            for part in parts {
                preds.extend(described(part)?);
            }
            Some(preds)
        }
        _ => None,
    }
}

/// "Whenever an artifact you control is put into a graveyard from the battlefield": a permanent that is no creature
/// does not die, the renderers' "dies" is the oracle's longer wording.
fn put_into_graveyard(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
        ..
    } = ability
    else {
        return;
    };
    let kinds: Vec<&Pred> = what
        .0
        .iter()
        .filter(|pred| matches!(pred, Pred::Type(_)))
        .collect();
    let creature = kinds
        .iter()
        .any(|pred| **pred == Pred::Type(CardType::Creature));
    if kinds.is_empty() || creature {
        return;
    }
    let longer: Vec<String> = texts
        .iter()
        .filter(|text| text.starts_with("Whenever ") && text.contains(" dies,"))
        .map(|text| {
            text.replacen(
                " dies,",
                " is put into a graveyard from the battlefield,",
                1,
            )
        })
        .collect();
    texts.extend(longer);
}

/// Whether the predicates describe the object by its own characteristics (not the source, the targets or the event).
fn plain(selector: &Selector) -> bool {
    !selector.0.is_empty()
        && selector.0.iter().all(|pred| {
            !matches!(
                pred,
                Pred::This | Pred::Is(_) | Pred::Targeting(_) | Pred::SharesName(_)
            )
        })
}
