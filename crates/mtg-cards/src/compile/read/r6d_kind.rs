//! "Destroy target artifact. If it was a creature, draw a card." (r6d): an `If` that counts whether the one target (or
//! the card itself) is of a kind, worded with "it", "that creature" or "~" and "is", "was" or "has". The count of one
//! object at one or more is "it is", at none "it isn't"; "that creature" names a noun the one target spec is.

use super::selectors::{Form, phrase_text};
use super::triggers::article;
use crate::refs::{Pred, Ref, TargetSpec, Targetable, ZoneKind};
use crate::value::{Amount, Cmp, Condition};

/// The nouns "that creature", "that card" may name the target by.
const NOUNS: [&str; 8] = [
    "creature",
    "permanent",
    "land",
    "artifact",
    "enchantment",
    "planeswalker",
    "card",
    "spell",
];

/// Whether the condition counts one object as being of a kind (`Some(true)`) or not (`Some(false)`).
fn kind_count(condition: &Condition) -> Option<(&Ref, &crate::refs::Selector, bool)> {
    let Condition::CountOf {
        subject,
        selector,
        op,
        right: Amount::Fixed(number),
    } = condition
    else {
        return None;
    };
    if selector.0.is_empty() || !matches!(subject, Ref::Target(0) | Ref::This) {
        return None;
    }
    let positive = match (op, number) {
        (Cmp::Ge | Cmp::Eq, 1) | (Cmp::Gt, 0) => true,
        (Cmp::Eq | Cmp::Le, 0) | (Cmp::Lt, 1) => false,
        _ => return None,
    };
    Some((subject, selector, positive))
}

/// The wordings of the condition after "if", whatever the targets; empty when it is not such a count.
pub(super) fn all_texts(condition: &Condition) -> Vec<String> {
    let Some((subject, selector, positive)) = kind_count(condition) else {
        return Vec::new();
    };
    let Some(phrase) = phrase_text(selector, Form::Subject) else {
        return Vec::new();
    };
    let subjects: Vec<String> = match subject {
        Ref::This => std::iter::once("~".to_owned())
            .chain(NOUNS.iter().map(|noun| format!("this {noun}")))
            .collect(),
        _ => std::iter::once("it".to_owned())
            .chain(NOUNS.iter().map(|noun| format!("that {noun}")))
            .collect(),
    };
    let verbs: &[&str] = if positive {
        &["is", "was"]
    } else {
        &["isn't", "wasn't"]
    };
    let mut kinds = vec![format!("{} {phrase}", article(&phrase))];
    let typed = !["permanent", "spell", "card"]
        .iter()
        .any(|noun| phrase.ends_with(noun));
    if typed {
        kinds.push(format!("{} {phrase} card", article(&phrase)));
    }
    // "legendary permanent", "red permanent": the adjective alone ("it's legendary"), the first wording.
    if let Some(adjective) = phrase.strip_suffix(" permanent")
        && !adjective.contains(' ')
        && !adjective.starts_with("non")
    {
        kinds.insert(0, adjective.to_owned());
    }
    let mut out = Vec::new();
    for subject in &subjects {
        for verb in verbs {
            for kind in &kinds {
                out.push(format!("{subject} {verb} {kind}"));
            }
        }
        if subject == "it" && positive {
            for kind in &kinds {
                out.push(format!("it's {kind}"));
            }
        }
        // "that land was nonbasic": none of the subject's own noun with the adjective.
        if let Some(noun) = subject
            .strip_prefix("that ")
            .or(subject.strip_prefix("this "))
            && let Some(adjective) = phrase.strip_suffix(&format!(" {noun}"))
            && !positive
            && !adjective.contains(' ')
            && !adjective.starts_with("non")
        {
            for verb in ["is", "was"] {
                out.push(format!("{subject} {verb} non{adjective}"));
            }
        }
        if !positive {
            continue;
        }
        // "its mana value was 3 or less": the one target or the card measured.
        if subject == "it" || subject == "~" {
            let owner = if subject == "it" { "its" } else { "~'s" };
            if let Some(rest) = phrase.strip_prefix("permanent with ")
                && let Some(stat) = ["mana value", "power", "toughness"]
                    .into_iter()
                    .find(|stat| rest.starts_with(&format!("{stat} ")))
            {
                let compared = &rest[stat.len() + 1..];
                for verb in ["is", "was"] {
                    out.push(format!("{owner} {stat} {verb} {compared}"));
                }
            }
        }
        // "has mana value 3 or less": a kind given by what it has, of the subject's own noun or any permanent.
        let has = phrase.strip_prefix("permanent with ").or_else(|| {
            let noun = subject.strip_prefix("that ")?;
            phrase.strip_prefix(&format!("{noun} with "))
        });
        if let Some(rest) = has {
            for verb in ["has", "had"] {
                out.push(format!("{subject} {verb} {rest}"));
            }
        }
    }
    out
}

/// The wording of an intervening "if" that the card itself is of a kind: "if ~ is tapped".
pub(super) fn own_text(condition: &Condition) -> Option<String> {
    let (subject, _, _) = kind_count(condition)?;
    (*subject == Ref::This).then_some(())?;
    all_texts(condition).into_iter().next()
}

/// The wordings of the condition that fit the ability's targets: "it" and "that creature" name the one target only
/// when there is one target spec of one object, and "that creature" only when that spec is a creature.
pub(super) fn texts(condition: &Condition, targets: &[TargetSpec]) -> Option<Vec<String>> {
    let (subject, _, _) = kind_count(condition)?;
    let all = all_texts(condition);
    if *subject == Ref::This {
        return Some(all);
    }
    let [spec] = targets else {
        return Some(Vec::new());
    };
    let one = spec.bounds.max == Some(Amount::Fixed(1));
    let Targetable::Object(selector) = &spec.what else {
        return Some(Vec::new());
    };
    if !one {
        return Some(Vec::new());
    }
    let fits = |noun: &str| {
        match noun {
        "permanent" => !selector
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::In(_) | Pred::Spell)),
        "card" => selector
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::In(zone) if *zone != ZoneKind::Battlefield)),
        "spell" => selector.0.contains(&Pred::Spell),
        _ => selector.0.iter().any(|pred| {
            matches!(pred, Pred::Type(kind) if format!("{kind:?}").eq_ignore_ascii_case(noun))
        }),
    }
    };
    Some(
        all.into_iter()
            .filter(|text| {
                text.strip_prefix("that ")
                    .and_then(|rest| rest.split(' ').next())
                    .is_none_or(fits)
            })
            .collect(),
    )
}
