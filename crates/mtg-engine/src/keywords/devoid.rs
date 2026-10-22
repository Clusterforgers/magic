//! Devoid (CR 702.114): a characteristic-defining ability that makes the object colorless in every zone.

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Ref, ZoneKind};

use super::KeywordRules;

/// The rules of devoid.
#[derive(Debug)]
pub(super) struct Devoid;

impl KeywordRules for Devoid {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Static {
            affected: Ref::This,
            mods: vec![Modification::SetColors(Vec::new())],
            condition: None,
            defining: true,
            from: ZoneKind::Battlefield,
        }]
    }
}
