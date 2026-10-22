//! Changeling (CR 702.73): a characteristic-defining ability that makes the object every creature type in every zone.

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Ref, ZoneKind};

use super::KeywordRules;

/// The rules of changeling.
#[derive(Debug)]
pub(super) struct Changeling;

impl KeywordRules for Changeling {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Static {
            affected: Ref::This,
            mods: vec![Modification::AddAllCreatureTypes],
            condition: None,
            defining: true,
            from: ZoneKind::Battlefield,
        }]
    }
}
