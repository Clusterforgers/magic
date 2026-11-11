//! Cleave (CR 702.148): "You may cast this spell by paying [cost] rather than paying its mana cost. If you do, remove the
//! words in square brackets from its text."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`, `CastWay::Cleave`). The keyword carries the spell its
//! text is without the bracketed words; a spell cast that way announces and resolves as that spell (`cleaved`, applied by
//! `resolve::spell_ability` and by the announcement once the way is chosen).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;

use super::KeywordRules;
use crate::ability_reader;
use crate::object::GameObject;

/// The rules of cleave.
#[derive(Debug)]
pub(super) struct Cleave;

impl KeywordRules for Cleave {}

/// The spell ability of `card` cast `way`: the cleaved spell its keyword carries when cast for the cleave cost, otherwise
/// `rules` as they are.
pub(crate) fn cleaved(
    card: &GameObject,
    way: Option<CastWay>,
    rules: Arc<Ability>,
) -> Arc<Ability> {
    if way != Some(CastWay::Cleave) || !matches!(*rules, Ability::Spell { .. }) {
        return rules;
    }
    ability_reader::printed_rules(card.card())
        .find_map(|rule| match &**rule {
            Ability::Keyword(Keyword::Cleave {
                targets, effects, ..
            }) => Some(Arc::new(Ability::Spell {
                targets: targets.clone(),
                effects: effects.clone(),
            })),
            _ => None,
        })
        .unwrap_or(rules)
}
