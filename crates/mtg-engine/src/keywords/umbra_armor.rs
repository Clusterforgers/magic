//! Umbra armor (CR 702.89): "If enchanted creature would be destroyed, instead remove all damage from it and destroy this
//! Aura."
//!
//! The keyword adds a replacement effect of the destruction of the permanent the Aura is attached to; the replacement step
//! does the removing of the damage and the destroying of the Aura (`replace.rs`).

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};

use super::KeywordRules;

/// The rules of umbra armor.
#[derive(Debug)]
pub(super) struct UmbraArmor;

impl KeywordRules for UmbraArmor {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::UmbraArmor) {
            return Vec::new();
        }
        vec![Ability::Replacement {
            when: ReplaceWhen::Destroyed {
                what: Selector(vec![Pred::Is(Ref::Attached)]),
            },
            by: ReplaceBy::UmbraArmor,
            condition: None,
            from: ZoneKind::Battlefield,
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: umbra armor adds one replacement of the destruction of the enchanted permanent that works from the battlefield, and nothing for another keyword.
    use super::*;

    #[test]
    fn umbra_armor_adds_a_replacement_of_the_destruction_of_the_host() {
        let added = UmbraArmor.adds_abilities(&Keyword::UmbraArmor);
        assert_eq!(
            added,
            [Ability::Replacement {
                when: ReplaceWhen::Destroyed {
                    what: Selector(vec![Pred::Is(Ref::Attached)]),
                },
                by: ReplaceBy::UmbraArmor,
                condition: None,
                from: ZoneKind::Battlefield,
            }]
        );
        assert_eq!(UmbraArmor.adds_abilities(&Keyword::Flying), []);
    }
}
