//! Persist (CR 702.79): "When this permanent dies, if it had no -1/-1 counters on it, return it to the battlefield under
//! its owner's control with a -1/-1 counter on it."

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;

use super::KeywordRules;
use super::undying::returns_with;

/// The rules of persist.
#[derive(Debug)]
pub(super) struct Persist;

impl KeywordRules for Persist {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        returns_with(CounterKind::MinusOneMinusOne)
    }
}
