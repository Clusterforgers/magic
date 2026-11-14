//! Suffix of the shared noun-phrase reader and writer for pack r60-03: "with total mana value 6 or less" and "with total
//! power X or less" (`Pred::TotalAtMost`), the most the cards chosen together may add up to.

use super::amounts::stat_name;
use super::tokens::Reader;
use crate::refs::{Pred, Selector, TargetSet, TargetSpec, Targetable};
use crate::value::{Amount, StatKind};

/// "with total mana value 6 or less" after the noun; X is the X of the sentence.
pub(super) fn r6003_suffix(r: &mut Reader<'_>) -> Option<Pred> {
    r.attempt(|r| {
        r.words("with total").then_some(())?;
        let stat = super::selectors::stat_word(r)?;
        matches!(stat, StatKind::ManaValue | StatKind::Power).then_some(())?;
        let most = if r.word("x") {
            Amount::X
        } else {
            Amount::Fixed(i32::try_from(r.number()?).ok()?)
        };
        r.words("or less").then_some(())?;
        Some(Pred::TotalAtMost { stat, most })
    })
}

/// The words of the suffix for a predicate of this module.
pub(super) fn r6003_suffix_text(pred: &Pred) -> Option<String> {
    let Pred::TotalAtMost { stat, most } = pred else {
        return None;
    };
    matches!(stat, StatKind::ManaValue | StatKind::Power).then_some(())?;
    let number = match most {
        Amount::Fixed(number) => number.to_string(),
        Amount::X => "X".to_owned(),
        _ => return None,
    };
    Some(format!("with total {} {number} or less", stat_name(*stat)?))
}

/// A target specification whose objects carry a total is the set of those objects under the total's rule
/// (`Targetable::Together`): the targets are judged together as they are chosen.
pub(super) fn r6003_targets_together(spec: TargetSpec) -> TargetSpec {
    let Targetable::Object(selector) = &spec.what else {
        return spec;
    };
    let Some((at, rule)) = selector
        .0
        .iter()
        .enumerate()
        .find_map(|(at, pred)| match pred {
            Pred::TotalAtMost {
                stat: StatKind::ManaValue,
                most,
            } => Some((at, TargetSet::TotalManaValueAtMost(most.clone()))),
            Pred::TotalAtMost {
                stat: StatKind::Power,
                most,
            } => Some((at, TargetSet::TotalPowerAtMost(most.clone()))),
            _ => None,
        })
    else {
        return spec;
    };
    let mut preds = selector.0.clone();
    preds.remove(at);
    TargetSpec {
        what: Targetable::Together {
            what: Box::new(Targetable::Object(Selector(preds))),
            rule,
        },
        ..spec
    }
}
