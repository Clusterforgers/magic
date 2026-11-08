//! The noun-phrase words of the filter atoms the Forge translator maps: "kicked" before the noun; after it "with the
//! greatest mana value" (an extreme among the phrase's own kind), "with a legendary Equipment attached to it", "with power
//! less than ~'s" and "with three or more charge counters on it".

use mtg_core::card::{CardType, Subtype, Supertype};

use super::amounts::stat_name;
use super::selectors::Form;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, StatKind};

/// An adjective of this module before the noun.
pub(super) fn vp20_adjective(word: &str) -> Option<Pred> {
    (word == "kicked").then_some(Pred::Kicked)
}

/// The adjective of this module for a predicate.
pub(super) fn vp20_adjective_text(pred: &Pred) -> Option<String> {
    (*pred == Pred::Kicked).then(|| "kicked".to_owned())
}

/// The kind an extreme is said among: the card types of the phrase, and "nonland" as the phrase says it.
fn kind_of(selector: &Selector) -> Vec<Pred> {
    selector
        .0
        .iter()
        .filter(|pred| match pred {
            Pred::Type(_) => true,
            Pred::Not(inner) => matches!(**inner, Pred::Type(CardType::Land)),
            _ => false,
        })
        .cloned()
        .collect()
}

/// Whether a phrase with an extreme says what it compares among: the phrase is its kind and the extreme alone ("creature
/// with the greatest mana value" is the greatest among creatures). A phrase with more ("artifact you control with the
/// greatest mana value" reads as among yours), an extreme among another kind or one after "a" has no words.
pub(super) fn vp20_fits(selector: &Selector, form: Form) -> bool {
    let extreme = |pred: &Pred| matches!(pred, Pred::GreatestAmong { .. } | Pred::LeastAmong { .. });
    if !selector.0.iter().any(extreme) {
        return true;
    }
    // After "a" the printed text names "the" one "or tied for" it, which no writer says.
    if matches!(form, Form::Subject | Form::Count) {
        return false;
    }
    let kind = kind_of(selector);
    let extremes: Vec<&Pred> = selector.0.iter().filter(|pred| extreme(pred)).collect();
    extremes.len() == 1
        && kind.len() + 1 == selector.0.len()
        && match extremes[0] {
            Pred::GreatestAmong { group, .. } | Pred::LeastAmong { group, .. } => {
                !group.0.is_empty() && group.0 == kind
            }
            _ => false,
        }
}

/// The words of an extreme of a stat: "greatest mana value", "lowest mana value", "least toughness".
fn extreme_text(stat: StatKind, greatest: bool) -> Option<String> {
    let name = stat_name(stat)?;
    let word = match (stat, greatest) {
                (StatKind::ManaValue, false) => "lowest",
        (_, true) => "greatest",
        (_, false) => "least",
    };
    Some(format!("with the {word} {name}"))
}

/// The suffix of this module for a predicate.
pub(super) fn vp20_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    let plural = form.plural();
    match pred {
        Pred::GreatestAmong { stat, .. } => extreme_text(*stat, true),
        Pred::LeastAmong { stat, .. } => extreme_text(*stat, false),
        // "with three or more charge counters on it".
        Pred::CounterCount {
            kind,
            op: Cmp::Ge,
            to,
        } if *to >= 2 => {
            let it = if plural { "them" } else { "it" };
            Some(format!(
                "with {} or more {} counters on {it}",
                super::amounts::count_word(*to)?,
                super::counters::kind_text(kind)?
            ))
        }
        Pred::HasAttached(held) if *held == legendary_equipment() => {
            let it = if plural { "them" } else { "it" };
            Some(format!("with a legendary Equipment attached to {it}"))
        }
        Pred::Test {
            left: Amount::Stat {
                of: Ref::Candidate,
                stat,
            },
            op: op @ (Cmp::Lt | Cmp::Gt),
            right:
                Amount::Stat {
                    of: Ref::This,
                    stat: same,
                },
        } if stat == same => {
            let more = if *op == Cmp::Lt { "less" } else { "greater" };
            Some(format!("with {} {more} than ~'s", stat_name(*stat)?))
        }
        _ => None,
    }
}

/// A legendary Equipment.
fn legendary_equipment() -> Selector {
    Selector(vec![
        Pred::Subtype(Subtype("Equipment".to_owned())),
        Pred::Supertype(Supertype::Legendary),
    ])
}

