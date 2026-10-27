//! Grants of the fifth sample of M to R: a quoted trigger the targets gain after an evasion ("Until end of turn, those
//! creatures gain "Whenever this creature deals combat damage to a player, draw a card.""), a copy that has a quoted
//! upkeep trigger ("except it has "At the beginning of your upkeep, if this creature isn't a token, ..."") and "have
//! equip {0} as long as ...".

use std::sync::Arc;

use super::{c5_mr_swapped, c5_mr_twin};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;
use crate::trigger::ReplaceBy;

/// The keyword equip {0}, and haste the renderers write in its place.
const C5_MR_EQUIP_ZERO: &str = r#"{"AddKeyword":{"Equip":"{0}"}}"#;
const C5_MR_HASTE: &str = r#"{"AddKeyword":"Haste"}"#;

/// The upkeep condition of a copy that is not a token.
const C5_MR_NOT_TOKEN: &str =
    r#""if":{"Not":{"Matches":{"subject":"This","selector":["Token"]}}},"#;

/// Adds the wordings of the grants.
pub(super) fn add(ability: &Ability, json: &str, texts: &mut Vec<String>) {
    c5_mr_then_gain(ability, texts);
    c5_mr_copy_has(ability, texts);
    let twin = c5_mr_twin(json, C5_MR_EQUIP_ZERO, C5_MR_HASTE);
    texts.extend(c5_mr_swapped(
        &twin,
        "quipment you control have haste as long as",
        "quipment you control have equip {0} as long as",
    ));
}

/// The wordings of a granted ability inside quotes, with the source written "this creature".
fn c5_mr_quoted(granted: &Ability) -> Vec<String> {
    let mut written = super::super::super::variants(granted);
    if written.is_empty() {
        written = c5_mr_unless_token(granted);
    }
    written
        .iter()
        .map(|text| format!("\"{}\"", text.replace('~', "this creature")))
        .collect()
}

/// "At the beginning of your upkeep, if ~ isn't a token, ..." from the twin without the condition.
fn c5_mr_unless_token(granted: &Ability) -> Vec<String> {
    let json = serde_json::to_string(granted).unwrap_or_default();
    c5_mr_twin(json.as_str(), C5_MR_NOT_TOKEN, "")
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", ")?;
            head.starts_with("At the beginning of ")
                .then(|| format!("{head}, if ~ isn't a token, {rest}"))
        })
        .collect()
}

/// "Until end of turn, those creatures gain "..."" after a first effect on the same targets.
fn c5_mr_then_gain(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { effects, .. } = ability else {
        return;
    };
    let [first, ..] = effects.as_slice() else {
        return;
    };
    let Some(Effect::Continuous {
        affected: Ref::Target(0),
        mods,
        duration: Duration::UntilEndOfTurn,
    }) = effects.last()
    else {
        return;
    };
    let [Modification::GrantAbility(granted)] = mods.as_slice() else {
        return;
    };
    if effects.len() != 2
        || !matches!(
            first,
            Effect::Continuous {
                affected: Ref::Target(0),
                ..
            }
        )
    {
        return;
    }
    let mut twin = ability.clone();
    if let Ability::Spell { effects, .. } = &mut twin {
        effects.pop();
    }
    let quoted = c5_mr_quoted(granted);
    let mut found = Vec::new();
    for head in super::super::super::variants(&twin) {
        for quote in &quoted {
            found.push(format!(
                "{head} Until end of turn, those creatures gain {quote}"
            ));
        }
    }
    texts.extend(found);
}

/// "You may have ~ enter as a copy of ..., except it has "..."" from the twin without the exception.
fn c5_mr_copy_has(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        by: ReplaceBy::AsCopy { except, .. },
        ..
    } = ability
    else {
        return;
    };
    let [Modification::GrantAbility(granted)] = except.as_slice() else {
        return;
    };
    let granted: Arc<Ability> = Arc::clone(granted);
    let mut twin = ability.clone();
    if let Ability::Replacement {
        by: ReplaceBy::AsCopy { except, .. },
        ..
    } = &mut twin
    {
        except.clear();
    }
    let quoted = c5_mr_quoted(&granted);
    let mut found = Vec::new();
    for head in super::super::super::variants(&twin) {
        let head = head.strip_suffix('.').unwrap_or(&head);
        for quote in &quoted {
            found.push(format!("{head}, except it has {quote}"));
        }
    }
    texts.extend(found);
}
