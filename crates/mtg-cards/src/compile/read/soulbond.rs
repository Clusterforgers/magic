//! Soulbond (CR 702.95): the condition "~ is paired with another creature" and the two creatures it names, "each of those
//! creatures" and "both creatures". Read from the wording and written back the same way.

use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;

/// The creature itself and the one it is paired with.
fn pair() -> Ref {
    Ref::Each(Selector(vec![Pred::AnyOf(vec![
        Pred::This,
        Pred::PairedWith(Ref::This),
    ])]))
}

/// Whether a static ability over `affected` may stand under `condition`: the pair only under the condition that the card is paired.
pub(super) fn allowed(affected: &Ref, condition: Option<&Condition>) -> bool {
    *affected != pair() || condition.is_some_and(|held| !condition_texts(held).is_empty())
}

/// "~ is paired with another creature": the card itself is paired.
pub(super) fn condition(r: &mut Reader<'_>) -> Option<Condition> {
    r.source().then_some(())?;
    r.need("is paired with another creature")?;
    Some(Condition::Matches {
        subject: Ref::This,
        selector: Selector(vec![Pred::Paired]),
    })
}

/// "~ is paired with another creature", the one wording of the condition.
pub(super) fn condition_texts(condition: &Condition) -> Vec<String> {
    match condition {
        Condition::Matches { subject, selector }
            if *subject == Ref::This && selector.0 == [Pred::Paired] =>
        {
            vec!["~ is paired with another creature".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// The pair a static ability changes: "each of those creatures" (one at a time, so a singular verb) or "both creatures".
pub(super) fn subject(r: &mut Reader<'_>) -> Option<(Ref, bool)> {
    if r.words("each of those creatures") {
        Some((pair(), false))
    } else if r.words("both creatures") {
        Some((pair(), true))
    } else {
        None
    }
}

/// The wordings of the pair as the subject of a sentence, with whether the verb is plural.
pub(super) fn subject_texts(affected: &Ref) -> Vec<(String, bool)> {
    if *affected == pair() {
        vec![
            ("Each of those creatures".to_owned(), false),
            ("Both creatures".to_owned(), true),
        ]
    } else {
        Vec::new()
    }
}
