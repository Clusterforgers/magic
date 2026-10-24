//! What a group or an Aura gives, part by part: a bonus, keywords, the loss of keywords or of every ability, base power
//! and toughness, a type or a subtype in addition to the others, and "is goaded", joined as the card words them
//! ("Nonartifact creatures get -1/-1 and lose all abilities", "Lands you control are every..."). Written from the data
//! only, for the combinations the fixed shapes of `statics::predicate` do not say.

use super::render::stats_text;
use super::selectors::{Form, granted_text, keyword_text, phrase_text, type_name};
use super::subtypes::plural_subtype;
use crate::modify::Modification;
use crate::value::Amount;

/// The predicates of a static ability over a group (`plural`) or one object, in every wording; empty when a
/// modification has no part here.
pub(super) fn predicates(mods: &[Modification], plural: bool) -> Vec<String> {
    let mut options: Vec<Vec<String>> = Vec::new();
    let mut rest = mods;
    while let Some((first, tail)) = rest.split_first() {
        let (taken, found) = match first {
            Modification::AddKeyword(_) | Modification::AddProtectionFromChosenColor => {
                let end = rest
                    .iter()
                    .position(|m| {
                        !matches!(
                            m,
                            Modification::AddKeyword(_)
                                | Modification::AddProtectionFromChosenColor
                        )
                    })
                    .unwrap_or(rest.len());
                let Some(given) = granted_text(&rest[..end]) else {
                    return Vec::new();
                };
                (
                    end,
                    vec![format!("{} {given}", verb(plural, "has", "have"))],
                )
            }
            _ => (1, single(first, tail.is_empty(), plural)),
        };
        if found.is_empty() {
            return Vec::new();
        }
        options.push(found);
        rest = &rest[taken..];
    }
    if options.is_empty() {
        return Vec::new();
    }
    joined(&options)
}

fn verb(plural: bool, one: &'static str, many: &'static str) -> &'static str {
    if plural { many } else { one }
}

/// One modification as a part of the predicate, in each wording.
fn single(modification: &Modification, _last: bool, plural: bool) -> Vec<String> {
    let (is, its) = if plural {
        ("are", "their")
    } else {
        ("is", "its")
    };
    match modification {
        Modification::ModifyPT { power, toughness } => bonus(power, toughness, plural),
        Modification::RemoveKeyword(keyword) => keyword_text(keyword)
            .map(|word| format!("{} {word}", verb(plural, "loses", "lose")))
            .into_iter()
            .collect(),
        Modification::LoseAbilities => {
            vec![format!("{} all abilities", verb(plural, "loses", "lose"))]
        }
        Modification::SetPT { power, toughness } => match (power, toughness) {
            (Amount::Fixed(power), Amount::Fixed(toughness)) => vec![format!(
                "{} base power and toughness {power}/{toughness}",
                verb(plural, "has", "have")
            )],
            _ => Vec::new(),
        },
        Modification::AddType(card_type) => {
            let name = type_name(*card_type);
            if name.is_empty() {
                return Vec::new();
            }
            let noun = if plural {
                format!("{name}s")
            } else {
                format!("{} {name}", super::triggers::article(name))
            };
            vec![format!("{is} {noun} in addition to {its} other types")]
        }
        Modification::AddSubtype(subtype) => {
            let name = &subtype.0;
            let noun = if plural {
                plural_subtype(name)
            } else {
                format!("{} {name}", super::triggers::article(name))
            };
            vec![
                format!("{is} {noun} in addition to {its} other types"),
                format!("{is} {noun} in addition to {its} other creature types"),
            ]
        }
        Modification::Goaded => vec![format!("{is} goaded")],
        Modification::CantBeBlockedBy(blockers) => phrase_text(blockers, Form::All)
            .map(|group| format!("can't be blocked by {group}"))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// "gets +2/+0" with the zero signed either way the cards sign it: "-2/-0" and "-2/+0".
fn bonus(power: &Amount, toughness: &Amount, plural: bool) -> Vec<String> {
    let Some((stats, tail)) = stats_text(power, toughness) else {
        return Vec::new();
    };
    let gets = verb(plural, "gets", "get");
    let mut found = vec![format!("{gets} {stats}{tail}")];
    if let Some((power, toughness)) = stats.split_once('/') {
        if power == "+0" && toughness.starts_with('-') {
            found.push(format!("{gets} -0/{toughness}{tail}"));
        } else if toughness == "+0" && power.starts_with('-') {
            found.push(format!("{gets} {power}/-0{tail}"));
        }
    }
    found
}

/// The parts in order, every choice of wording: two are joined by "and", more by commas and a last "and".
fn joined(options: &[Vec<String>]) -> Vec<String> {
    let mut texts = vec![Vec::<&str>::new()];
    for choices in options {
        texts = texts
            .iter()
            .flat_map(|so_far| {
                choices.iter().map(move |part| {
                    let mut next = so_far.clone();
                    next.push(part.as_str());
                    next
                })
            })
            .take(64)
            .collect();
    }
    texts
        .into_iter()
        .map(|parts| match parts.as_slice() {
            [rest @ .., last] if rest.len() >= 2 => format!("{}, and {last}", rest.join(", ")),
            _ => parts.join(" and "),
        })
        .collect()
}
