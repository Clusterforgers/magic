//! The object a "where X is" clause names: "where X is the power of the creature that died" for the creature of a death
//! trigger and "where X is that artifact's mana value" for the one target of a single card type.

use super::super::super::selectors::type_name;
use crate::ability::Ability;
use crate::refs::{Pred, Selector, TargetSpec, Targetable, ZoneKind};
use crate::trigger::TriggerPattern;

/// The characteristics a clause names.
const STATS: [&str; 3] = ["power", "toughness", "mana value"];

/// Adds the wordings that name the creature that died or the card type of the target.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut found = Vec::new();
    if dies(ability) {
        for stat in STATS {
            let said = format!(", where X is that creature's {stat}");
            let died = format!(", where X is the {stat} of the creature that died");
            found.extend(
                texts
                    .iter()
                    .filter(|t| t.contains(&said))
                    .map(|t| t.replace(&said, &died)),
            );
        }
    }
    if let Some(noun) = target_noun(ability) {
        for stat in STATS {
            let said = format!(", where X is its {stat}");
            let named = format!(", where X is that {noun}'s {stat}");
            found.extend(
                texts
                    .iter()
                    .filter(|t| t.contains(&said))
                    .map(|t| t.replace(&said, &named)),
            );
        }
    }
    texts.extend(found);
}

/// Whether the ability triggers when a permanent is put into a graveyard from the battlefield.
fn dies(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
                ..
            },
            ..
        }
    )
}

/// The card type of the first target when it is the only part of what the target must be.
fn target_noun(ability: &Ability) -> Option<&'static str> {
    let [
        TargetSpec {
            what: Targetable::Object(Selector(preds)),
            ..
        },
        ..,
    ] = ability.targets()
    else {
        return None;
    };
    let [Pred::Type(card_type)] = preds.as_slice() else {
        return None;
    };
    let name = type_name(*card_type);
    (!name.is_empty()).then_some(name)
}
