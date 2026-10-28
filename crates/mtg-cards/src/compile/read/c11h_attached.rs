//! Conditions on what an attached permanent is: "As long as enchanted creature is a Human, ...", "As long as enchanted
//! permanent is a creature, it gets +2/+2.", "As long as equipped creature is a Human or an Angel, ...", "As long as
//! enchanted creature is red or green, ..." and "As long as equipped creature isn't legendary, ...". Read from the
//! wording and written back the same way.

use super::selectors::{color, color_name};
use super::statics::{one_kind, single_texts};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;

/// The nouns an Aura or Equipment calls the permanent it is attached to.
const C11H_ATTACHED: [&str; 5] = [
    "equipped creature",
    "enchanted creature",
    "enchanted permanent",
    "enchanted land",
    "enchanted artifact",
];

/// "enchanted permanent is a creature", "equipped creature is a Human or an Angel", "enchanted creature is red or green",
/// "equipped creature isn't legendary", and the same after "it" when it names the attached permanent.
pub(super) fn c11h_read(r: &mut Reader<'_>, it: Option<&Ref>) -> Option<Condition> {
    let attached = C11H_ATTACHED.iter().any(|noun| r.words(noun))
        || (it == Some(&Ref::Attached) && r.word("it"));
    attached.then_some(())?;
    let pred = if r.word("isn't") {
        let word = r.take_word()?;
        Pred::Not(Box::new(c11h_state(word)?))
    } else {
        r.word("is").then_some(())?;
        c11h_kind(r)?
    };
    Some(Condition::Matches {
        subject: Ref::Attached,
        selector: Selector(vec![pred]),
    })
}

/// "a creature", "an Equipment", "a Human or an Angel", "red or green".
fn c11h_kind(r: &mut Reader<'_>) -> Option<Pred> {
    if r.word("a") || r.word("an") {
        let Selector(mut preds) = one_kind(r)?;
        let [_] = preds.as_slice() else {
            return None;
        };
        return preds.pop();
    }
    let first = color(r.take_word()?)?;
    r.word("or").then_some(())?;
    let second = color(r.take_word()?)?;
    Some(Pred::AnyOf(vec![Pred::Color(first), Pred::Color(second)]))
}

/// A state a permanent is or isn't in: "legendary", "tapped", "attacking", or a color.
fn c11h_state(word: &str) -> Option<Pred> {
    match word {
        "legendary" => Some(Pred::Supertype(mtg_core::card::Supertype::Legendary)),
        "tapped" => Some(Pred::Tapped),
        "untapped" => Some(Pred::Untapped),
        "attacking" => Some(Pred::Attacking),
        other => color(other).map(Pred::Color),
    }
}

/// The name of a state `c11h_state` reads.
fn c11h_state_text(pred: &Pred) -> Option<String> {
    Some(match pred {
        Pred::Supertype(mtg_core::card::Supertype::Legendary) => "legendary".to_owned(),
        Pred::Tapped => "tapped".to_owned(),
        Pred::Untapped => "untapped".to_owned(),
        Pred::Attacking => "attacking".to_owned(),
        Pred::Color(shade) => color_name(*shade).to_owned(),
        _ => return None,
    })
}

/// What `c11h_read` reads, written back with each noun for the attached permanent and with "it".
pub(super) fn c11h_texts(condition: &Condition) -> Vec<String> {
    let Condition::Matches {
        subject: Ref::Attached,
        selector: Selector(preds),
    } = condition
    else {
        return Vec::new();
    };
    let [pred] = preds.as_slice() else {
        return Vec::new();
    };
    let tails: Vec<String> = match pred {
        Pred::Not(inner) => c11h_state_text(inner)
            .map(|state| format!("isn't {state}"))
            .into_iter()
            .collect(),
        Pred::AnyOf(colors) => match colors.as_slice() {
            [Pred::Color(first), Pred::Color(second)] => vec![format!(
                "is {} or {}",
                color_name(*first),
                color_name(*second)
            )],
            _ => single_texts(&Selector(vec![pred.clone()]))
                .into_iter()
                .map(|kind| format!("is {kind}"))
                .collect(),
        },
        // A single state word of a creature is written by `as_long_as::state_texts`.
        _ if c11h_state_text(pred).is_some() => {
            return c11h_state_text(pred)
                .map(|state| {
                    C11H_ATTACHED[2..]
                        .iter()
                        .map(|noun| format!("{noun} is {state}"))
                        .collect()
                })
                .unwrap_or_default();
        }
        _ => single_texts(&Selector(vec![pred.clone()]))
            .into_iter()
            .map(|kind| format!("is {kind}"))
            .collect(),
    };
    C11H_ATTACHED
        .iter()
        .chain(std::iter::once(&"it"))
        .flat_map(|noun| tails.iter().map(move |tail| format!("{noun} {tail}")))
        .collect()
}
