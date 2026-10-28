//! Read ahead (CR 702.155): "As this Saga enters, choose a chapter number. It enters with that many lore counters, and
//! chapter abilities with a lower number don't trigger."
//!
//! The keyword adds an `Enters` replacement that asks for the chapter (`EntersChoice::Chapter`); the choice turns the
//! lore counter the Saga enters with into that many (`replace.rs`) and is remembered, so the trigger detection skips
//! the chapters below it (`saga::w2b_read_past`).

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Selector, ZoneKind};
use mtg_cards::trigger::{EntersChoice, ReplaceBy, ReplaceWhen};

use super::KeywordRules;

/// The rules of read ahead.
#[derive(Debug)]
pub(super) struct ReadAhead;

impl KeywordRules for ReadAhead {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::ReadAhead) {
            return Vec::new();
        }
        vec![Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: Selector::this(),
            },
            by: ReplaceBy::Choose(EntersChoice::Chapter),
            condition: None,
            from: ZoneKind::Battlefield,
        }]
    }
}
