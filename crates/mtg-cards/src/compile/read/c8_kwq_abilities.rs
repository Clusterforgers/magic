//! A counter of an ability on the stack, in any ability: "counter target spell, activated ability, or triggered
//! ability", "counter target activated or triggered ability", "counter target activated ability" (CR 701.6b, 113.7).

use super::Reader;
use crate::refs::{AbilityKind, Bounds, Pred, Ref, Selector, TargetSpec, Targetable};

/// The target phrases of an ability on the stack, each with what it may target.
fn c8_kwq_phrases() -> [(&'static str, Vec<Pred>); 4] {
    let activated = Pred::Ability(AbilityKind::Activated);
    let triggered = Pred::Ability(AbilityKind::Triggered);
    [
        (
            "spell, activated ability, or triggered ability",
            vec![Pred::AnyOf(vec![
                Pred::Spell,
                activated.clone(),
                triggered.clone(),
            ])],
        ),
        (
            "activated or triggered ability",
            vec![Pred::AnyOf(vec![activated.clone(), triggered.clone()])],
        ),
        ("activated ability", vec![activated]),
        ("triggered ability", vec![triggered]),
    ]
}

/// "target activated ability" and the other phrases: the one target of the clause, `Target(0)`.
pub(super) fn c8_kwq_counter_target(r: &mut Reader<'_>) -> Option<Ref> {
    if !r.targets.is_empty() {
        return None;
    }
    r.need("target")?;
    let what = c8_kwq_phrases()
        .into_iter()
        .find_map(|(words, preds)| c8_kwq_read(r, words).then_some(preds))?;
    r.targets.push(TargetSpec {
        what: Targetable::Object(Selector(what)),
        bounds: Bounds::default(),
        divide: None,
    });
    Some(Ref::Target(0))
}

/// Reads the words of a phrase, a comma after a word included, all or none.
fn c8_kwq_read(r: &mut Reader<'_>, phrase: &str) -> bool {
    r.attempt(|r| {
        phrase
            .split(' ')
            .all(|word| match word.strip_suffix(',') {
                Some(word) => r.word(word) && r.punct(','),
                None => r.word(word),
            })
            .then_some(())
    })
    .is_some()
}

/// The words of a target that is an ability on the stack: "spell, activated ability, or triggered ability".
pub(super) fn c8_kwq_ability_phrase(selector: &Selector) -> Option<&'static str> {
    c8_kwq_phrases()
        .into_iter()
        .find_map(|(words, preds)| (selector.0 == preds).then_some(words))
}
