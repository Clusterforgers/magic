//! Cascade (CR 702.85): "When you cast this spell, exile cards from the top of your library until you exile a nonland
//! card that costs less. You may cast it without paying its mana cost. Put all cards exiled this way that were not cast
//! on the bottom of your library in a random order."
//!
//! The keyword adds a trigger that works from the stack and runs the steps `effect::cascade` lists: the exiling, the
//! cast during the resolution (`Effect::CastFromZone`) and the move to the bottom. Each cascade is its own trigger, so a
//! spell with two of them cascades twice, and the cards cast by one cascade trigger the next (CR 702.85c).

use mtg_cards::ability::Ability;
use mtg_cards::effect::cascade;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;

use super::KeywordRules;

/// The rules of cascade.
#[derive(Debug)]
pub(super) struct Cascade;

impl KeywordRules for Cascade {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector::this(),
                by: Ref::You,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Stack,
            targets: Vec::new(),
            effects: cascade(),
        }]
    }
}
