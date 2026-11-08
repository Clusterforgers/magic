//! Oracle wordings of sentences the renderers write another way: "becomes an artifact creature with power and toughness
//! each equal to its mana value until end of turn" for an X/X where X is its mana value; "you may have it deal damage
//! equal to its power to target creature. If you do, ~ assigns no combat damage this turn." for the optional blow that
//! takes the creature's combat damage; "another red source you control" for a bonus to the damage of any red object,
//! spells too; and "Gain control of all artifacts your opponents control until end of turn. Untap them." for the group
//! named once as control changes.

use super::standin::with_effects;
use crate::ability::Ability;
use crate::compile::read::selectors::{Form, keyword_text, phrase_text};
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

/// The X/X sentence as the renderers write it, around the types the permanent becomes.
const SIZED: (&str, &str) = (
    "becomes an X/X ",
    "creature until end of turn, where X is its mana value.",
);

/// The optional blow as the renderers write it, around its target.
const BLOW: (&str, &str) = (
    "you may ~ deals damage equal to its power to ",
    " and ~ assigns no combat damage this turn.",
);

/// Adds the oracle wordings of the sentences.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if let Some(sentence) = taken_group(ability.effects()).filter(|_| ability.targets().is_empty()) {
        // A loyalty ability keeps the cost the renderers wrote before its colon.
        let loyal: Vec<String> = match ability {
            Ability::Loyalty { .. } => texts
                .iter()
                .filter_map(|text| text.split_once(": "))
                .map(|(cost, _)| format!("{cost}: {}.", super::super::capitalize(&sentence)))
                .collect(),
            _ => with_effects(ability, &sentence),
        };
        texts.extend(loyal);
    }
    let source = any_source(ability);
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            each_equal(text)
                .or_else(|| have_it_deal(text))
                .or_else(|| source.then(|| red_source(text)).flatten())
        })
        .collect();
    texts.extend(found);
}

/// "gain control of all artifacts your opponents control until end of turn. Untap them. They gain haste until end of
/// turn": the group is named once, as control changes, and the later steps act on the permanents taken.
fn taken_group(effects: &[Effect]) -> Option<String> {
    let (group, them, rest) = match effects {
        [
            Effect::Bind {
                what: Ref::Each(group),
                bind,
                adds: false,
            },
            rest @ ..,
        ] => (group, Ref::Bound(bind.clone()), rest),
        // A group named again at each step is the same permanents only when control does not decide it.
        [
            Effect::Continuous {
                affected: Ref::Each(group),
                ..
            },
            ..,
        ] if !reads_control(group) => (group, Ref::Each(group.clone()), effects),
        _ => return None,
    };
    let [
        Effect::Continuous {
            affected,
            mods: control,
            duration: Duration::UntilEndOfTurn,
        },
        rest @ ..,
    ] = rest
    else {
        return None;
    };
    let ([Modification::SetController(Ref::You)], true) =
        (control.as_slice(), *affected == them)
    else {
        return None;
    };
    let (untap, grant) = match rest {
        [Effect::Untap { what }, grant @ ..] if *what == them => (true, grant),
        _ => (false, rest),
    };
    let keywords = match grant {
        [] => None,
        [Effect::Continuous {
            affected,
            mods,
            duration: Duration::UntilEndOfTurn,
        }] if *affected == them => Some(
            mods.iter()
                .map(|change| match change {
                    Modification::AddKeyword(keyword) => keyword_text(keyword),
                    _ => None,
                })
                .collect::<Option<Vec<String>>>()?,
        ),
        _ => return None,
    };
    let mut sentence = format!(
        "gain control of all {} until end of turn",
        phrase_text(group, Form::All)?
    );
    if untap {
        sentence.push_str(". Untap them");
    }
    if let Some(keywords) = keywords {
        let [keyword] = keywords.as_slice() else {
            return None;
        };
        sentence.push_str(&format!(". They gain {keyword} until end of turn"));
    }
    Some(sentence)
}

/// Whether a group is read by who controls its objects.
fn reads_control(group: &crate::refs::Selector) -> bool {
    serde_json::to_string(group)
        .is_ok_and(|text| text.contains("Controlled") || text.contains("Controller"))
}

/// Whether the ability adds to the damage of a source named by no type or zone, so a spell is one too.
fn any_source(ability: &Ability) -> bool {
    let Ok(value) = serde_json::to_value(ability) else {
        return false;
    };
    let Some(preds) = value
        .pointer("/Replacement/when/Damage/source")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    value.pointer("/Replacement/by/Add").is_some()
        && value.pointer("/Replacement/when/Damage/to").is_none()
        && !preds.is_empty()
        && preds.iter().all(|pred| {
            pred.get("Type").is_none() && pred.get("Subtype").is_none() && pred.get("In").is_none()
        })
}

/// "If another red source you control would deal damage to a permanent or player, it deals that much damage plus 1 to
/// that permanent or player instead."
fn red_source(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(" permanent you control would deal damage to a permanent or player, it deals that much damage plus ")?;
    let (amount, tail) = rest.split_once(" instead.")?;
    (!amount.contains(' ')).then_some(())?;
    Some(format!(
        "{head} source you control would deal damage to a permanent or player, it deals that much damage plus {amount} to that permanent or player instead.{tail}"
    ))
}

/// "becomes an artifact creature with power and toughness each equal to its mana value until end of turn."
fn each_equal(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(SIZED.0)?;
    let (types, tail) = rest.split_once(SIZED.1)?;
    (!types.contains(['.', ',', '"'])).then_some(())?;
    let article = if types.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    Some(format!(
        "{head}becomes {article} {types}creature with power and toughness each equal to its mana value until end of turn.{tail}"
    ))
}

/// "you may have it deal damage equal to its power to target creature. If you do, ~ assigns no combat damage this turn."
fn have_it_deal(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(BLOW.0)?;
    let (target, tail) = rest.split_once(BLOW.1)?;
    target.starts_with("target ").then_some(())?;
    (!target.contains(['.', ','])).then_some(())?;
    Some(format!(
        "{head}you may have it deal damage equal to its power to {target}. If you do, ~ assigns no combat damage this turn.{tail}"
    ))
}
